using Microsoft.AspNetCore.Mvc;
using Teleport.Net;

namespace Teleport.Net.AspNetCore;

internal static class TeleportDiscovery
{
    public static IReadOnlyList<TeleportProcedureDescriptor> Discover(IEnumerable<Assembly> assemblies)
    {
        var discovered = new List<TeleportProcedureDescriptor>();
        var seenRoutes = new HashSet<string>(StringComparer.Ordinal);
        var nullability = new NullabilityInfoContext();

        foreach (var assembly in assemblies)
        {
            foreach (var type in assembly.DefinedTypes)
            {
                var module = type.GetCustomAttribute<TeleportModuleAttribute>();
                var hasProcedureMethods = type.DeclaredMethods.Any(HasProcedureAttribute);
                if (hasProcedureMethods && module is null)
                {
                    throw new InvalidOperationException(
                        $"type {type.FullName} declares teleport methods but is missing [TeleportModule]");
                }

                if (module is null)
                {
                    continue;
                }

                if (!(type.IsClass && type.IsAbstract && type.IsSealed && (type.IsPublic || type.IsNestedPublic)))
                {
                    throw new InvalidOperationException(
                        $"type {type.FullName} must be public static to use [TeleportModule]");
                }

                foreach (var method in type.DeclaredMethods)
                {
                    var procedure = BuildDescriptor(module.Namespace, method, nullability);
                    if (procedure is null)
                    {
                        continue;
                    }

                    if (!seenRoutes.Add(procedure.Route))
                    {
                        throw new InvalidOperationException($"duplicate teleport route discovered: {procedure.Route}");
                    }

                    discovered.Add(procedure);
                }
            }
        }

        return discovered
            .OrderBy(p => p.Namespace, StringComparer.Ordinal)
            .ThenBy(p => p.MethodName, StringComparer.Ordinal)
            .ToArray();
    }

    private static bool HasProcedureAttribute(MethodInfo method)
    {
        return method.GetCustomAttribute<TeleportQueryAttribute>() is not null
            || method.GetCustomAttribute<TeleportCommandAttribute>() is not null
            || method.GetCustomAttribute<TeleportFormAttribute>() is not null;
    }

    private static TeleportProcedureDescriptor? BuildDescriptor(
        string moduleNamespace,
        MethodInfo method,
        NullabilityInfoContext nullability)
    {
        if (!method.IsPublic || !method.IsStatic || method.IsGenericMethodDefinition || method.IsSpecialName)
        {
            return null;
        }

        var kind = GetKind(method);
        if (kind is null)
        {
            return null;
        }

        var methodName = method.GetCustomAttribute<TeleportNameAttribute>()?.Name
            ?? JsonNamingPolicy.CamelCase.ConvertName(method.Name);

        ValidateReturnType(moduleNamespace, methodName, method.ReturnType);

        var parameters = method.GetParameters();
        ParameterInfo? payloadParameter = null;
        ParameterInfo? authParameter = null;
        var services = new List<ParameterInfo>();

        foreach (var parameter in parameters)
        {
            if (parameter.GetCustomAttribute<FromServicesAttribute>() is not null)
            {
                services.Add(parameter);
                continue;
            }

            if (parameter.ParameterType == typeof(HttpContext) || parameter.ParameterType == typeof(CancellationToken))
            {
                continue;
            }

            if (IsAuthParameter(parameter))
            {
                if (authParameter is not null)
                {
                    throw new InvalidOperationException(
                        $"procedure {moduleNamespace}.{methodName} has more than one auth parameter");
                }

                authParameter = parameter;
                continue;
            }

            if (payloadParameter is not null)
            {
                throw new InvalidOperationException(
                    $"procedure {moduleNamespace}.{methodName} has more than one payload parameter");
            }

            payloadParameter = parameter;
        }

        if (kind == TeleportProcedureKind.Query && payloadParameter is not null && IsPrimitiveQueryInput(payloadParameter.ParameterType))
        {
            throw new InvalidOperationException(
                $"procedure {moduleNamespace}.{methodName} query inputs must be struct/class wrappers so parameter names survive export");
        }

        var authRequired = authParameter is not null && !IsNullableClaimsPrincipal(authParameter, nullability);
        var authOptional = authParameter is not null && !authRequired;

        var httpMethod = kind == TeleportProcedureKind.Query ? HttpMethods.Get : HttpMethods.Post;
        var route = $"/rpc/{moduleNamespace}.{methodName}";
        var payloadType = payloadParameter?.ParameterType ?? typeof(Unit);

        return new TeleportProcedureDescriptor(
            moduleNamespace,
            methodName,
            route,
            httpMethod,
            kind.Value,
            method,
            payloadParameter,
            payloadType,
            authParameter,
            authRequired,
            authOptional,
            services.ToArray(),
            parameters);
    }

    private static TeleportProcedureKind? GetKind(MethodInfo method)
    {
        if (method.GetCustomAttribute<TeleportQueryAttribute>() is not null)
        {
            return TeleportProcedureKind.Query;
        }

        if (method.GetCustomAttribute<TeleportCommandAttribute>() is not null)
        {
            return TeleportProcedureKind.Command;
        }

        if (method.GetCustomAttribute<TeleportFormAttribute>() is not null)
        {
            return TeleportProcedureKind.Form;
        }

        return null;
    }

    private static bool IsAuthParameter(ParameterInfo parameter)
    {
        if (parameter.ParameterType == typeof(ClaimsPrincipal))
        {
            return true;
        }

        if (parameter.GetCustomAttribute<TeleportAuthAttribute>() is not null)
        {
            throw new InvalidOperationException(
                $"auth parameter '{parameter.Name}' must be of type ClaimsPrincipal");
        }

        return false;
    }

    private static bool IsNullableClaimsPrincipal(ParameterInfo parameter, NullabilityInfoContext nullability)
    {
        if (parameter.ParameterType != typeof(ClaimsPrincipal))
        {
            return false;
        }

        return nullability.Create(parameter).ReadState == NullabilityState.Nullable;
    }

    private static void ValidateReturnType(string moduleNamespace, string methodName, Type returnType)
    {
        if (returnType == typeof(void) || returnType == typeof(Task) || returnType == typeof(ValueTask))
        {
            throw new InvalidOperationException(
                $"procedure {moduleNamespace}.{methodName} must return TeleportResult<TOutput, TError> or Task<TeleportResult<TOutput, TError>>");
        }

        if (returnType.IsGenericType && returnType.GetGenericTypeDefinition() == typeof(Task<>))
        {
            returnType = returnType.GetGenericArguments()[0];
        }

        if (!returnType.IsGenericType || returnType.GetGenericTypeDefinition() != typeof(TeleportResult<,>))
        {
            throw new InvalidOperationException(
                $"procedure {moduleNamespace}.{methodName} must return TeleportResult<TOutput, TError> or Task<TeleportResult<TOutput, TError>>");
        }
    }

    private static bool IsPrimitiveQueryInput(Type type)
    {
        type = Nullable.GetUnderlyingType(type) ?? type;
        return type == typeof(string) || type.IsPrimitive || type.IsEnum || type == typeof(decimal);
    }
}

internal readonly struct Unit
{
}
