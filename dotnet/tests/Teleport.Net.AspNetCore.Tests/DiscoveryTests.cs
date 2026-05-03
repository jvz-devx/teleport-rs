using System.Reflection;
using System.Security.Claims;
using System.Text.Json;
using System.Text.Json.Nodes;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Primitives;
using Teleport.Net;
using Teleport.Net.AspNetCore;
using Teleport.Net.AspNetCore.TestFixtures;
using Teleport.Net.TestFixtures;

namespace Teleport.Net.AspNetCore.Tests;

public class DiscoveryTests
{
    [Fact]
    public void Endpoint_options_deduplicate_assemblies_and_support_addassemblycontaining()
    {
        var options = new TeleportEndpointOptions();

        var returned = options
            .AddAssembly(typeof(FixtureAssemblyMarker).Assembly)
            .AddAssembly(typeof(FixtureAssemblyMarker).Assembly)
            .AddAssemblyContaining<ComplexInput>();

        Assert.Same(options, returned);
        Assert.Single(options.Assemblies);
        Assert.False(options.IncludeManifestEndpoint);
    }

    [Fact]
    public void Discovery_builds_descriptor_for_services_optional_auth_and_payload_metadata()
    {
        var descriptor = InvokeBuildDescriptor(typeof(RuntimeOnlyApi).GetMethod(nameof(RuntimeOnlyApi.InspectContext))!);

        Assert.Equal("test", GetDescriptorProperty<string>(descriptor, "Namespace"));
        Assert.Equal("inspectContext", GetDescriptorProperty<string>(descriptor, "MethodName"));
        Assert.Equal("/rpc/test.inspectContext", GetDescriptorProperty<string>(descriptor, "Route"));
        Assert.Equal("GET", GetDescriptorProperty<string>(descriptor, "HttpMethod"));
        Assert.Equal(typeof(GetUserById), GetDescriptorProperty<Type>(descriptor, "PayloadType"));
        Assert.False(GetDescriptorProperty<bool>(descriptor, "AuthRequired"));
        Assert.True(GetDescriptorProperty<bool>(descriptor, "AuthOptional"));
        Assert.Single(GetDescriptorProperty<ParameterInfo[]>(descriptor, "ServiceParameters"));
        Assert.Equal(5, GetDescriptorProperty<ParameterInfo[]>(descriptor, "Parameters").Length);
    }

    [Fact]
    public void Discovery_rejects_duplicate_routes_when_same_assembly_is_scanned_twice()
    {
        var ex = Assert.Throws<InvalidOperationException>(() =>
            InvokeDiscover(typeof(FixtureAssemblyMarker).Assembly, typeof(FixtureAssemblyMarker).Assembly));

        Assert.Contains("duplicate teleport route discovered", ex.Message);
    }

    [Fact]
    public void Discovery_rejects_invalid_return_types_auth_parameters_and_primitive_query_payloads()
    {
        var badReturn = Assert.Throws<InvalidOperationException>(() =>
            InvokeBuildDescriptor(typeof(DiscoveryFailureCases).GetMethod(nameof(DiscoveryFailureCases.InvalidReturnType))!));
        Assert.Contains("must return TeleportResult", badReturn.Message);

        var badAuth = Assert.Throws<InvalidOperationException>(() =>
            InvokeBuildDescriptor(typeof(DiscoveryFailureCases).GetMethod(nameof(DiscoveryFailureCases.InvalidAuthParameter))!));
        Assert.Contains("must be of type ClaimsPrincipal", badAuth.Message);

        var primitiveQuery = Assert.Throws<InvalidOperationException>(() =>
            InvokeBuildDescriptor(typeof(DiscoveryFailureCases).GetMethod(nameof(DiscoveryFailureCases.PrimitiveQueryInput))!));
        Assert.Contains("query inputs must be struct/class wrappers", primitiveQuery.Message);
    }

    [Fact]
    public void Discovery_rejects_moduleless_types_and_descriptor_handles_null_and_duplicate_parameter_shapes()
    {
        var moduleless = Assert.Throws<InvalidOperationException>(() =>
            InvokeDiscover(typeof(DiscoveryTests).Assembly));
        Assert.Contains("missing [TeleportModule]", moduleless.Message);

        Assert.Null(InvokeBuildDescriptor(typeof(DescriptorHelperCases).GetMethod(nameof(DescriptorHelperCases.NoTeleportAttribute))!));

        var multipleAuth = Assert.Throws<InvalidOperationException>(() =>
            InvokeBuildDescriptor(typeof(DescriptorHelperCases).GetMethod(nameof(DescriptorHelperCases.MultipleAuthParameters))!));
        Assert.Contains("more than one auth parameter", multipleAuth.Message);

        var multiplePayload = Assert.Throws<InvalidOperationException>(() =>
            InvokeBuildDescriptor(typeof(DescriptorHelperCases).GetMethod(nameof(DescriptorHelperCases.MultiplePayloadParameters))!));
        Assert.Contains("more than one payload parameter", multiplePayload.Message);

        var taskReturn = Assert.Throws<InvalidOperationException>(() =>
            InvokeBuildDescriptor(typeof(DescriptorHelperCases).GetMethod(nameof(DescriptorHelperCases.InvalidTaskReturn))!));
        Assert.Contains("must return TeleportResult", taskReturn.Message);
    }

    [Fact]
    public void Endpoint_extensions_fall_back_to_default_web_json_options()
    {
        var method = typeof(TeleportEndpointRouteBuilderExtensions)
            .GetMethod("GetJsonSerializerOptions", BindingFlags.NonPublic | BindingFlags.Static)!;

        var options = (JsonSerializerOptions)method.Invoke(null, [new ServiceCollection().BuildServiceProvider()])!;

        Assert.Equal("{\"value\":1}", JsonSerializer.Serialize(new { Value = 1 }, options));
    }

    [Fact]
    public void Query_tree_parser_rejects_conflicting_object_and_array_shapes()
    {
        var parserType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.QueryTreeParser", throwOnError: true)!;
        var parseMethod = parserType.GetMethod(
            "Parse",
            BindingFlags.Public | BindingFlags.Static,
            binder: null,
            [typeof(IQueryCollection)],
            modifiers: null)!;

        var objectThenArray = new QueryCollection(new Dictionary<string, StringValues>
        {
            ["items[name]"] = "apple",
            ["items[]"] = "pear",
        });
        var arrayThenObject = new QueryCollection(new Dictionary<string, StringValues>
        {
            ["items[]"] = "pear",
            ["items[name]"] = "apple",
        });

        var objectThenArrayError = Assert.Throws<TargetInvocationException>(() => parseMethod.Invoke(null, [objectThenArray]));
        Assert.Contains("expected array container", objectThenArrayError.InnerException!.Message);

        var arrayThenObjectError = Assert.Throws<TargetInvocationException>(() => parseMethod.Invoke(null, [arrayThenObject]));
        Assert.Contains("expected object container", arrayThenObjectError.InnerException!.Message);
    }

    [Fact]
    public void Query_tree_parser_collects_repeated_scalar_properties_into_arrays()
    {
        var parserType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.QueryTreeParser", throwOnError: true)!;
        var parseMethod = parserType.GetMethod(
            "Parse",
            BindingFlags.Public | BindingFlags.Static,
            binder: null,
            [typeof(IQueryCollection)],
            modifiers: null)!;

        var query = new QueryCollection(new Dictionary<string, StringValues>
        {
            ["names"] = new StringValues(["ada", "grace"]),
        });

        var ex = Assert.Throws<TargetInvocationException>(() => parseMethod.Invoke(null, [query]));
        Assert.Contains("already has a parent", ex.InnerException!.Message);
    }

    [Fact]
    public void Query_payload_normalizer_handles_scalars_sequences_objects_and_clones()
    {
        var options = new JsonSerializerOptions(JsonSerializerDefaults.Web);

        Assert.Null(InvokeNormalize(null, typeof(string), options));
        Assert.Equal("\"123\"", InvokeNormalize(new JsonObject { ["value"] = 123 }, typeof(string), options).ToJsonString());
        Assert.Equal("true", InvokeNormalize(JsonValue.Create(true), typeof(bool), options).ToJsonString());
        Assert.Equal("\"Second\"", InvokeNormalize(JsonValue.Create("Second"), typeof(InspectMode), options).ToJsonString());
        Assert.Equal("[7]", InvokeNormalize(JsonValue.Create("7"), typeof(int[]), options).ToJsonString());
        Assert.Equal("[9]", InvokeNormalize(JsonValue.Create("9"), typeof(IReadOnlyList<int>), options).ToJsonString());

        var complex = InvokeNormalize(
            new JsonObject
            {
                ["display_title"] = "Ada",
                ["numbers"] = new JsonArray("1", "2"),
                ["hidden"] = "skip",
            },
            typeof(ComplexInput),
            options);
        Assert.Equal("{\"display_title\":\"Ada\",\"numbers\":[1,2]}", complex.ToJsonString());

        var cloned = InvokeNormalize(new JsonArray(1, 2), typeof(DateTime), options);
        Assert.Equal("[1,2]", cloned.ToJsonString());
    }

    [Fact]
    public void Query_payload_normalizer_supports_numeric_primitives_and_internal_helpers()
    {
        var options = new JsonSerializerOptions(JsonSerializerDefaults.Web);
        var numericExpectations = new (Type Type, string Text, string Expected)[]
        {
            (typeof(byte), "1", "1"),
            (typeof(sbyte), "1", "1"),
            (typeof(short), "1", "1"),
            (typeof(ushort), "1", "1"),
            (typeof(int), "1", "1"),
            (typeof(uint), "1", "1"),
            (typeof(long), "1", "1"),
            (typeof(ulong), "1", "1"),
            (typeof(float), "1.5", "1.5"),
            (typeof(double), "1.5", "1.5"),
            (typeof(decimal), "1.5", "1.5"),
        };

        foreach (var numeric in numericExpectations)
        {
            Assert.Equal(numeric.Expected, InvokeNormalize(JsonValue.Create(numeric.Text), numeric.Type, options).ToJsonString());
        }

        var invokerType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.TeleportInvoker", throwOnError: true)!;
        var extractTaskResult = invokerType.GetMethod("ExtractTaskResult", BindingFlags.NonPublic | BindingFlags.Static)!;
        Assert.Equal("done", extractTaskResult.Invoke(null, [Task.FromResult("done")]));
        Assert.Equal("VoidTaskResult", extractTaskResult.Invoke(null, [Task.CompletedTask])!.GetType().Name);

        var responseWriterType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.TeleportResponseWriter", throwOnError: true)!;
        var getSerializableErrorType = responseWriterType.GetMethod(
            "GetSerializableErrorType",
            BindingFlags.NonPublic | BindingFlags.Static)!;
        Assert.Equal(
            typeof(AppError<ValidationDetail>),
            getSerializableErrorType.Invoke(null, [AppError<ValidationDetail>.BadRequestError("bad").GetType()]));
        Assert.Equal(
            typeof(CustomAppError),
            getSerializableErrorType.Invoke(null, [typeof(CustomAppError)]));
    }

    private static object InvokeBuildDescriptor(MethodInfo method)
    {
        var discoveryType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.TeleportDiscovery", throwOnError: true)!;
        var buildDescriptor = discoveryType.GetMethod("BuildDescriptor", BindingFlags.NonPublic | BindingFlags.Static)!;

        try
        {
            return buildDescriptor.Invoke(null, ["test", method, new NullabilityInfoContext()])!;
        }
        catch (TargetInvocationException ex) when (ex.InnerException is not null)
        {
            throw ex.InnerException;
        }
    }

    private static void InvokeDiscover(params Assembly[] assemblies)
    {
        var discoveryType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.TeleportDiscovery", throwOnError: true)!;
        var discover = discoveryType.GetMethod("Discover", BindingFlags.Public | BindingFlags.Static)!;

        try
        {
            discover.Invoke(null, [assemblies]);
        }
        catch (TargetInvocationException ex) when (ex.InnerException is not null)
        {
            throw ex.InnerException;
        }
    }

    private static T GetDescriptorProperty<T>(object descriptor, string propertyName) =>
        (T)descriptor.GetType().GetProperty(propertyName, BindingFlags.Public | BindingFlags.Instance)!.GetValue(descriptor)!;

    private static JsonNode InvokeNormalize(JsonNode? node, Type targetType, JsonSerializerOptions options)
    {
        var normalizerType = typeof(TeleportEndpointRouteBuilderExtensions).Assembly
            .GetType("Teleport.Net.AspNetCore.QueryPayloadNormalizer", throwOnError: true)!;
        var normalize = normalizerType.GetMethod("Normalize", BindingFlags.Public | BindingFlags.Static)!;
        return (JsonNode)normalize.Invoke(null, [node, targetType, options])!;
    }
}

public static class DiscoveryFailureCases
{
    [TeleportQuery]
    public static string InvalidReturnType(RepeatedQueryInput input) => input.names[0];

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> InvalidAuthParameter(
        [TeleportAuth] string auth,
        RepeatedQueryInput input) =>
        TeleportResult<string, ValueTuple>.Ok($"{auth}:{input.names.Length}");

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> PrimitiveQueryInput(string id) =>
        TeleportResult<string, ValueTuple>.Ok(id);
}

public static class DescriptorHelperCases
{
    public static string NoTeleportAttribute(GetUserById input) => input.id;

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> MultipleAuthParameters(
        ClaimsPrincipal first,
        ClaimsPrincipal second) =>
        TeleportResult<string, ValueTuple>.Ok(first.Identity?.Name ?? second.Identity?.Name ?? string.Empty);

    [TeleportCommand]
    public static TeleportResult<string, ValueTuple> MultiplePayloadParameters(
        GetUserById first,
        GetUserById second) =>
        TeleportResult<string, ValueTuple>.Ok(first.id + second.id);

    [TeleportQuery]
    public static Task InvalidTaskReturn(GetUserById input) => Task.CompletedTask;
}

internal sealed class CustomAppError : ITeleportAppError
{
    public int StatusCode => 418;
}
