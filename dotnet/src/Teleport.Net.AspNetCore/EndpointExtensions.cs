using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Microsoft.AspNetCore.Http.Json;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Options;
using Teleport.Net;

namespace Teleport.Net.AspNetCore;

public static class TeleportServiceCollectionExtensions
{
    public static IServiceCollection AddTeleport(this IServiceCollection services)
    {
        ArgumentNullException.ThrowIfNull(services);

        services.TryAddSingleton<TeleportRuntimeMarker>();
        return services;
    }
}

public static class TeleportEndpointRouteBuilderExtensions
{
    public static IEndpointRouteBuilder MapTeleportEndpoints(
        this IEndpointRouteBuilder endpoints,
        Action<TeleportEndpointOptions>? configure = null)
    {
        ArgumentNullException.ThrowIfNull(endpoints);

        var options = new TeleportEndpointOptions();
        configure?.Invoke(options);

        var jsonOptions = GetJsonSerializerOptions(endpoints.ServiceProvider);
        var procedures = TeleportDiscovery.Discover(options.Assemblies);

        if (options.IncludeManifestEndpoint && procedures.Any(procedure => procedure.Route == "/rpc/__manifest"))
        {
            throw new InvalidOperationException("teleport manifest endpoint conflicts with a procedure route named /rpc/__manifest");
        }

        foreach (var procedure in procedures)
        {
            endpoints.MapMethods(
                procedure.Route,
                [procedure.HttpMethod],
                context => TeleportInvoker.InvokeAsync(context, procedure, jsonOptions));
        }

        if (options.IncludeManifestEndpoint)
        {
            endpoints.MapGet("/rpc/__manifest", async context =>
            {
                context.Response.StatusCode = StatusCodes.Status200OK;
                context.Response.ContentType = "application/json; charset=utf-8";
                await JsonSerializer.SerializeAsync(
                    context.Response.Body,
                    TeleportManifestDocument.FromProcedures(procedures),
                    typeof(TeleportManifestDocument),
                    jsonOptions,
                    context.RequestAborted).ConfigureAwait(false);
            });
        }

        return endpoints;
    }

    private static JsonSerializerOptions GetJsonSerializerOptions(IServiceProvider services)
    {
        var options = services
            .GetService<IOptions<Microsoft.AspNetCore.Http.Json.JsonOptions>>()?
            .Value.SerializerOptions;
        if (options is not null)
        {
            return options;
        }

        return new JsonSerializerOptions(JsonSerializerDefaults.Web);
    }
}

internal sealed class TeleportRuntimeMarker;

internal enum TeleportProcedureKind
{
    Query,
    Command,
    Form,
}

internal sealed record TeleportProcedureDescriptor(
    string Namespace,
    string MethodName,
    string Route,
    string HttpMethod,
    TeleportProcedureKind Kind,
    MethodInfo Method,
    ParameterInfo? PayloadParameter,
    Type PayloadType,
    ParameterInfo? AuthParameter,
    bool AuthRequired,
    bool AuthOptional,
    ParameterInfo[] ServiceParameters,
    ParameterInfo[] Parameters);

internal sealed record TeleportManifestDocument
{
    [JsonPropertyName("procedures")]
    public Dictionary<string, TeleportManifestEntry> Procedures { get; init; } = new(StringComparer.Ordinal);

    public static TeleportManifestDocument FromProcedures(IReadOnlyList<TeleportProcedureDescriptor> procedures)
    {
        var document = new TeleportManifestDocument();

        foreach (var procedure in procedures)
        {
            document.Procedures[procedure.Route[5..]] = new TeleportManifestEntry
            {
                Method = procedure.HttpMethod,
                Path = procedure.Route,
            };
        }

        return document;
    }
}

internal sealed record TeleportManifestEntry
{
    [JsonPropertyName("method")]
    public string Method { get; init; } = string.Empty;

    [JsonPropertyName("path")]
    public string Path { get; init; } = string.Empty;
}

internal static class TeleportInvoker
{
    public static async Task InvokeAsync(
        HttpContext context,
        TeleportProcedureDescriptor procedure,
        JsonSerializerOptions jsonOptions)
    {
        object?[] arguments;

        try
        {
            arguments = await BuildArgumentsAsync(context, procedure, jsonOptions).ConfigureAwait(false);
        }
        catch (AppErrorException ex)
        {
            await TeleportResponseWriter.WriteErrorAsync(context, ex.Error, jsonOptions).ConfigureAwait(false);
            return;
        }
        catch (Exception ex)
        {
            var logger = context.RequestServices.GetService<ILoggerFactory>()?.CreateLogger("Teleport.Net.AspNetCore");
            logger?.LogError(ex, "failed to bind request for {Route}", procedure.Route);
            await TeleportResponseWriter.WriteErrorAsync(
                context,
                AppError<object>.InternalError("internal server error"),
                jsonOptions).ConfigureAwait(false);
            return;
        }

        object? result;
        try
        {
            result = procedure.Method.Invoke(null, arguments);
        }
        catch (Exception ex)
        {
            var logger = context.RequestServices.GetService<ILoggerFactory>()?.CreateLogger("Teleport.Net.AspNetCore");
            logger?.LogError(ex, "procedure {Route} threw", procedure.Route);
            await TeleportResponseWriter.WriteErrorAsync(
                context,
                AppError<object>.InternalError("internal server error"),
                jsonOptions).ConfigureAwait(false);
            return;
        }

        if (result is Task task)
        {
            try
            {
                await task.ConfigureAwait(false);
            }
            catch (Exception ex)
            {
                var logger = context.RequestServices.GetService<ILoggerFactory>()?.CreateLogger("Teleport.Net.AspNetCore");
                logger?.LogError(ex, "procedure {Route} threw", procedure.Route);
                await TeleportResponseWriter.WriteErrorAsync(
                    context,
                    AppError<object>.InternalError("internal server error"),
                    jsonOptions).ConfigureAwait(false);
                return;
            }

            result = ExtractTaskResult(task);
        }

        if (result is not ITeleportResult teleportResult)
        {
            throw new InvalidOperationException(
                $"procedure {procedure.Route} must return TeleportResult<TOutput, TError> or Task<TeleportResult<TOutput, TError>>");
        }

        if (teleportResult.IsSuccess)
        {
            await TeleportResponseWriter.WriteSuccessAsync(context, teleportResult.Value, jsonOptions).ConfigureAwait(false);
            return;
        }

        if (teleportResult.Error is null)
        {
            throw new InvalidOperationException($"procedure {procedure.Route} returned a failure without an AppError");
        }

        await TeleportResponseWriter.WriteErrorAsync(context, teleportResult.Error, jsonOptions).ConfigureAwait(false);
    }

    private static object? ExtractTaskResult(Task task)
    {
        var taskType = task.GetType();
        if (taskType.IsGenericType)
        {
            return taskType.GetProperty("Result")?.GetValue(task);
        }

        return null;
    }

    private static async Task<object?[]> BuildArgumentsAsync(
        HttpContext context,
        TeleportProcedureDescriptor procedure,
        JsonSerializerOptions jsonOptions)
    {
        var parameters = procedure.Parameters;
        var arguments = new object?[parameters.Length];

        for (var i = 0; i < parameters.Length; i++)
        {
            var parameter = parameters[i];
            arguments[i] = await BindParameterAsync(context, procedure, parameter, jsonOptions).ConfigureAwait(false);
        }

        return arguments;
    }

    private static async Task<object?> BindParameterAsync(
        HttpContext context,
        TeleportProcedureDescriptor procedure,
        ParameterInfo parameter,
        JsonSerializerOptions jsonOptions)
    {
        if (parameter == procedure.AuthParameter)
        {
            var user = context.User;
            if (procedure.AuthRequired && user.Identity?.IsAuthenticated != true)
            {
                throw new AppErrorException(AppError<object>.UnauthorizedError());
            }

            if (!procedure.AuthRequired && user.Identity?.IsAuthenticated != true)
            {
                return null;
            }

            return user;
        }

        if (parameter.ParameterType == typeof(HttpContext))
        {
            return context;
        }

        if (parameter.ParameterType == typeof(CancellationToken))
        {
            return context.RequestAborted;
        }

        if (parameter.GetCustomAttribute<FromServicesAttribute>() is not null)
        {
            var service = context.RequestServices.GetService(parameter.ParameterType);
            if (service is null)
            {
                throw new InvalidOperationException(
                    $"service parameter '{parameter.Name}' on {procedure.Route} could not be resolved");
            }

            return service;
        }

        if (parameter == procedure.PayloadParameter)
        {
            return await BindPayloadAsync(context, procedure, parameter, jsonOptions).ConfigureAwait(false);
        }

        throw new InvalidOperationException($"unsupported parameter '{parameter.Name}' on {procedure.Route}");
    }

    private static async Task<object?> BindPayloadAsync(
        HttpContext context,
        TeleportProcedureDescriptor procedure,
        ParameterInfo parameter,
        JsonSerializerOptions jsonOptions)
    {
        try
        {
            if (procedure.Kind == TeleportProcedureKind.Query)
            {
                var tree = QueryTreeParser.Parse(context.Request.Query);
                return DeserializeQueryOrFormPayload(tree, parameter.ParameterType, jsonOptions);
            }

            if (procedure.Kind == TeleportProcedureKind.Form && context.Request.HasFormContentType)
            {
                var form = await context.Request.ReadFormAsync(context.RequestAborted).ConfigureAwait(false);
                var tree = QueryTreeParser.Parse(form);
                return DeserializeQueryOrFormPayload(tree, parameter.ParameterType, jsonOptions);
            }

            return await JsonSerializer.DeserializeAsync(
                context.Request.Body,
                parameter.ParameterType,
                jsonOptions,
                context.RequestAborted).ConfigureAwait(false);
        }
        catch (JsonException ex)
        {
            throw new AppErrorException(AppError<object>.BadRequestError($"invalid request payload: {ex.Message}"));
        }
        catch (FormatException ex)
        {
            throw new AppErrorException(AppError<object>.BadRequestError($"invalid request payload: {ex.Message}"));
        }
        catch (InvalidOperationException ex)
        {
            throw new AppErrorException(AppError<object>.BadRequestError($"invalid request payload: {ex.Message}"));
        }
    }

    private static object? DeserializeQueryOrFormPayload(
        JsonNode tree,
        Type targetType,
        JsonSerializerOptions jsonOptions)
    {
        var normalized = QueryPayloadNormalizer.Normalize(tree, targetType, jsonOptions);
        return normalized.Deserialize(targetType, jsonOptions);
    }
}

internal sealed class AppErrorException : Exception
{
    public AppErrorException(ITeleportAppError error)
    {
        Error = error;
    }

    public ITeleportAppError Error { get; }
}

internal static class TeleportResponseWriter
{
    public static async Task WriteSuccessAsync(
        HttpContext context,
        object? value,
        JsonSerializerOptions jsonOptions)
    {
        context.Response.StatusCode = StatusCodes.Status200OK;
        context.Response.ContentType = "application/json; charset=utf-8";

        await JsonSerializer.SerializeAsync(
            context.Response.Body,
            value is ValueTuple ? null : value,
            value is ValueTuple ? typeof(object) : value?.GetType() ?? typeof(object),
            jsonOptions,
            context.RequestAborted).ConfigureAwait(false);
    }

    public static async Task WriteErrorAsync(
        HttpContext context,
        ITeleportAppError error,
        JsonSerializerOptions jsonOptions)
    {
        context.Response.StatusCode = error.StatusCode;
        context.Response.ContentType = "application/json; charset=utf-8";

        await JsonSerializer.SerializeAsync(
            context.Response.Body,
            error,
            GetSerializableErrorType(error.GetType()),
            jsonOptions,
            context.RequestAborted).ConfigureAwait(false);
    }

    private static Type GetSerializableErrorType(Type concreteType)
    {
        for (var current = concreteType; current is not null; current = current.BaseType)
        {
            if (current.IsGenericType && current.GetGenericTypeDefinition() == typeof(AppError<>))
            {
                return current;
            }
        }

        return concreteType;
    }
}

internal static class QueryPayloadNormalizer
{
    public static JsonNode Normalize(JsonNode? node, Type targetType, JsonSerializerOptions jsonOptions)
    {
        var normalized = NormalizeNode(node, targetType, jsonOptions);
        return normalized ?? JsonValue.Create((string?)null)!;
    }

    private static JsonNode? NormalizeNode(JsonNode? node, Type targetType, JsonSerializerOptions jsonOptions)
    {
        var underlying = Nullable.GetUnderlyingType(targetType);
        if (underlying is not null)
        {
            targetType = underlying;
        }

        if (node is JsonObject objectNode && IsScalarLike(targetType) && objectNode.Count == 1)
        {
            node = objectNode.First().Value;
        }

        if (node is null)
        {
            return null;
        }

        if (targetType == typeof(string))
        {
            return JsonValue.Create(NodeAsString(node));
        }

        if (targetType == typeof(bool))
        {
            return JsonValue.Create(bool.Parse(NodeAsString(node)));
        }

        if (targetType.IsEnum)
        {
            return JsonValue.Create(NodeAsString(node));
        }

        if (TryConvertNumber(node, targetType, out var numeric))
        {
            return numeric;
        }

        if (TryGetSequenceElementType(targetType, out var elementType))
        {
            if (node is not JsonArray arrayNode)
            {
                arrayNode = new JsonArray(node);
            }

            var normalizedArray = new JsonArray();
            foreach (var item in arrayNode)
            {
                normalizedArray.Add(NormalizeNode(item, elementType, jsonOptions));
            }

            return normalizedArray;
        }

        if (node is JsonObject sourceObject)
        {
            var normalizedObject = new JsonObject();
            foreach (var property in GetSerializableProperties(targetType, jsonOptions))
            {
                if (sourceObject.TryGetPropertyValue(property.JsonName, out var child))
                {
                    normalizedObject[property.JsonName] = NormalizeNode(child, property.PropertyType, jsonOptions);
                }
            }

            return normalizedObject;
        }

        return node.DeepClone();
    }

    private static bool IsScalarLike(Type type) =>
        type == typeof(string) || type.IsPrimitive || type.IsEnum || type == typeof(decimal);

    private static string NodeAsString(JsonNode node)
    {
        if (node is JsonValue value)
        {
            if (value.TryGetValue<string>(out var stringValue))
            {
                return stringValue!;
            }

            if (value.TryGetValue<bool>(out var boolValue))
            {
                return boolValue ? "true" : "false";
            }

            if (value.TryGetValue<int>(out var intValue))
            {
                return intValue.ToString(System.Globalization.CultureInfo.InvariantCulture);
            }

            if (value.TryGetValue<long>(out var longValue))
            {
                return longValue.ToString(System.Globalization.CultureInfo.InvariantCulture);
            }

            if (value.TryGetValue<decimal>(out var decimalValue))
            {
                return decimalValue.ToString(System.Globalization.CultureInfo.InvariantCulture);
            }

            return value.ToJsonString().Trim('"');
        }

        return node.ToJsonString().Trim('"');
    }

    private static bool TryConvertNumber(JsonNode node, Type targetType, out JsonNode? normalized)
    {
        normalized = null;
        var text = NodeAsString(node);
        var style = System.Globalization.NumberStyles.Float | System.Globalization.NumberStyles.AllowThousands;
        var culture = System.Globalization.CultureInfo.InvariantCulture;

        try
        {
            if (targetType == typeof(byte))
            {
                normalized = JsonValue.Create(byte.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(sbyte))
            {
                normalized = JsonValue.Create(sbyte.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(short))
            {
                normalized = JsonValue.Create(short.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(ushort))
            {
                normalized = JsonValue.Create(ushort.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(int))
            {
                normalized = JsonValue.Create(int.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(uint))
            {
                normalized = JsonValue.Create(uint.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(long))
            {
                normalized = JsonValue.Create(long.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(ulong))
            {
                normalized = JsonValue.Create(ulong.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(float))
            {
                normalized = JsonValue.Create(float.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(double))
            {
                normalized = JsonValue.Create(double.Parse(text, style, culture));
                return true;
            }
            if (targetType == typeof(decimal))
            {
                normalized = JsonValue.Create(decimal.Parse(text, style, culture));
                return true;
            }
        }
        catch (FormatException)
        {
            return false;
        }

        return false;
    }

    private static bool TryGetSequenceElementType(Type type, out Type elementType)
    {
        if (type.IsArray)
        {
            elementType = type.GetElementType()!;
            return true;
        }

        if (type.IsGenericType)
        {
            var definition = type.GetGenericTypeDefinition();
            if (definition == typeof(List<>) ||
                definition == typeof(IReadOnlyList<>) ||
                definition == typeof(IList<>) ||
                definition == typeof(IEnumerable<>))
            {
                elementType = type.GetGenericArguments()[0];
                return true;
            }
        }

        elementType = null!;
        return false;
    }

    private static IReadOnlyList<SerializableProperty> GetSerializableProperties(Type type, JsonSerializerOptions jsonOptions)
    {
        return type.GetProperties(BindingFlags.Public | BindingFlags.Instance)
            .Where(property => property.GetMethod is not null && property.GetMethod.IsPublic)
            .Where(property => property.GetIndexParameters().Length == 0)
            .Where(property => property.GetCustomAttribute<JsonIgnoreAttribute>() is null)
            .Select(property => new SerializableProperty(
                property.GetCustomAttribute<JsonPropertyNameAttribute>()?.Name
                    ?? jsonOptions.PropertyNamingPolicy?.ConvertName(property.Name)
                    ?? property.Name,
                property.PropertyType))
            .ToArray();
    }

    private sealed record SerializableProperty(string JsonName, Type PropertyType);
}
