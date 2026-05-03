using System.Diagnostics;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Security.Claims;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Teleport.Net;

public static class TeleportContractExporter
{
    public static ContractBundle Build(Assembly assembly, JsonSerializerOptions? jsonOptions = null)
    {
        ArgumentNullException.ThrowIfNull(assembly);

        var context = new ExportContext(jsonOptions);
        foreach (var type in GetLoadableTypes(assembly))
        {
            if (!IsTeleportModule(type, out var moduleName))
            {
                continue;
            }

            foreach (var method in type.GetMethods(BindingFlags.Public | BindingFlags.Static | BindingFlags.DeclaredOnly))
            {
                if (method.IsSpecialName)
                {
                    continue;
                }

                var procAttr = GetProcedureKind(method);
                if (procAttr is null)
                {
                    continue;
                }

                context.Procedures.Add(ExportProcedure(moduleName, method, procAttr, context));
            }
        }

        context.Sort();
        return context.ToBundle();
    }

    public static string ExportJson(Assembly assembly, JsonSerializerOptions? jsonOptions = null)
    {
        var options = CreateJsonOptions();
        return JsonSerializer.Serialize(Build(assembly, jsonOptions), options);
    }

    private static JsonSerializerOptions CreateJsonOptions()
    {
        var options = new JsonSerializerOptions
        {
            WriteIndented = true,
        };
        return options;
    }

    private static ProcedureContract ExportProcedure(
        string moduleName,
        MethodInfo method,
        ProcedureKindAttribute procAttr,
        ExportContext context)
    {
        var methodName = GetMethodName(method);
        var path = $"/rpc/{moduleName}.{methodName}";

        var payloadParameters = new List<ParameterInfo>();
        ParameterInfo? authParameter = null;

        foreach (var parameter in method.GetParameters())
        {
            if (IsAuthParameter(parameter))
            {
                if (authParameter is not null)
                {
                    throw context.Error(method, "multiple auth parameters are not supported");
                }

                EnsureAuthParameterType(parameter, context);
                authParameter = parameter;
                continue;
            }

            if (IsFromServices(parameter))
            {
                continue;
            }

            payloadParameters.Add(parameter);
        }

        if (payloadParameters.Count > 1)
        {
            throw context.Error(method, "only one non-auth payload parameter is supported");
        }

        var inputParameter = payloadParameters.Count == 1 ? payloadParameters[0] : null;
        if (procAttr.Kind == ProcedureKind.Query && inputParameter is not null && IsPrimitiveQueryInput(inputParameter.ParameterType))
        {
            throw context.Error(
                method,
                "query inputs must be struct/class wrappers so the contract can preserve parameter names");
        }

        var inputType = inputParameter is null
            ? new TypeExpr.Tuple(Array.Empty<TypeExpr>())
            : context.ExportType(inputParameter.ParameterType, method);

        var authMode = authParameter is null
            ? AuthMode.None
            : IsNullableClaimsPrincipal(authParameter, context)
                ? AuthMode.Optional
                : AuthMode.Required;

        var (outputType, errorType) = context.ExportReturnTypes(method);

        return new ProcedureContract
        {
            Name = $"{moduleName}.{methodName}",
            Namespace = moduleName,
            MethodName = methodName,
            ProcedureKind = procAttr.Kind,
            HttpMethod = procAttr.Kind == ProcedureKind.Query ? HttpMethod.Get : HttpMethod.Post,
            Path = path,
            InputEncoding = inputParameter is null
                ? InputEncoding.None
                : procAttr.Kind switch
                {
                    ProcedureKind.Query => InputEncoding.QueryString,
                    ProcedureKind.Command => InputEncoding.JsonBody,
                    ProcedureKind.Form => InputEncoding.FormBody,
                    _ => throw context.Error(method, $"unsupported procedure kind {procAttr.Kind}")
                },
            AuthMode = authMode,
            Doc = GetDoc(method),
            InputType = inputType,
            OutputType = outputType,
            ErrorType = errorType,
        };
    }

    private static IEnumerable<Type> GetLoadableTypes(Assembly assembly)
    {
        try
        {
            return assembly.GetTypes();
        }
        catch (ReflectionTypeLoadException ex)
        {
            return ex.Types.Where(type => type is not null).Cast<Type>();
        }
    }

    private static bool IsTeleportModule(Type type, out string moduleName)
    {
        moduleName = string.Empty;
        if (!type.IsClass || !type.IsAbstract || !type.IsSealed || !(type.IsPublic || type.IsNestedPublic))
        {
            return false;
        }

        var attr = type.GetCustomAttribute<TeleportModuleAttribute>();
        if (attr is null)
        {
            return false;
        }

        moduleName = attr.Namespace?.Trim() ?? string.Empty;
        return moduleName.Length > 0;
    }

    private static ProcedureKindAttribute? GetProcedureKind(MethodInfo method)
    {
        var kinds = new List<ProcedureKindAttribute>();
        if (method.GetCustomAttribute<TeleportQueryAttribute>() is not null)
        {
            kinds.Add(new ProcedureKindAttribute(ProcedureKind.Query));
        }
        if (method.GetCustomAttribute<TeleportCommandAttribute>() is not null)
        {
            kinds.Add(new ProcedureKindAttribute(ProcedureKind.Command));
        }
        if (method.GetCustomAttribute<TeleportFormAttribute>() is not null)
        {
            kinds.Add(new ProcedureKindAttribute(ProcedureKind.Form));
        }

        if (kinds.Count > 1)
        {
            throw new TeleportContractExportException(method, "only one of [TeleportQuery], [TeleportCommand], or [TeleportForm] may be applied");
        }

        return kinds.Count == 0 ? null : kinds[0];
    }

    private static string GetMethodName(MethodInfo method)
    {
        var overrideName = method.GetCustomAttribute<TeleportNameAttribute>()?.Name?.Trim();
        if (!string.IsNullOrWhiteSpace(overrideName))
        {
            return overrideName!;
        }

        return JsonNamingPolicy.CamelCase.ConvertName(method.Name);
    }

    private static bool IsAuthParameter(ParameterInfo parameter) =>
        parameter.GetCustomAttribute<TeleportAuthAttribute>() is not null ||
        parameter.ParameterType == typeof(ClaimsPrincipal);

    private static void EnsureAuthParameterType(ParameterInfo parameter, ExportContext context)
    {
        var parameterType = Nullable.GetUnderlyingType(parameter.ParameterType) ?? parameter.ParameterType;
        if (parameterType != typeof(ClaimsPrincipal))
        {
            throw context.Error(parameter.Member, "TeleportAuth can only be applied to ClaimsPrincipal or ClaimsPrincipal?");
        }
    }

    private static bool IsNullableClaimsPrincipal(ParameterInfo parameter, ExportContext context)
    {
        var parameterType = Nullable.GetUnderlyingType(parameter.ParameterType) ?? parameter.ParameterType;
        if (parameterType != typeof(ClaimsPrincipal))
        {
            throw context.Error(parameter.Member, "TeleportAuth can only be applied to ClaimsPrincipal or ClaimsPrincipal?");
        }

        var info = context.Nullability.Create(parameter);
        return info.ReadState == NullabilityState.Nullable;
    }

    private static bool IsFromServices(ParameterInfo parameter) =>
        parameter.CustomAttributes.Any(attr =>
            attr.AttributeType.FullName == "Microsoft.AspNetCore.Mvc.FromServicesAttribute");

    private static bool IsPrimitiveQueryInput(Type type)
    {
        type = Nullable.GetUnderlyingType(type) ?? type;

        return type == typeof(string) || type.IsPrimitive || type.IsEnum || type == typeof(decimal);
    }

    private static string GetDoc(MemberInfo? member) =>
        member?.GetCustomAttribute<TeleportDocAttribute>()?.Text ?? string.Empty;

    private sealed record ProcedureKindAttribute(ProcedureKind Kind);

    private sealed class ExportContext(JsonSerializerOptions? jsonOptions)
    {
        private readonly NullabilityInfoContext _nullability = new();
        private readonly Dictionary<Type, TypeExpr> _exportedTypes = new();
        private readonly Dictionary<Type, string> _namedTypeNames = new();
        private readonly HashSet<Type> _namedTypeInProgress = new();
        private readonly List<NamedTypeContract> _namedTypes = new();

        public List<ProcedureContract> Procedures { get; } = [];

        public NullabilityInfoContext Nullability => _nullability;

        private JsonNamingPolicy NamingPolicy => jsonOptions?.PropertyNamingPolicy ?? JsonNamingPolicy.CamelCase;

        public void Sort()
        {
            Procedures.Sort((a, b) => string.CompareOrdinal(a.Name, b.Name));
            _namedTypes.Sort((a, b) => string.CompareOrdinal(a.Name, b.Name));
        }

        public ContractBundle ToBundle() => new()
        {
            Version = TeleportContractSchema.Version,
            Procedures = Procedures,
            Types = _namedTypes,
        };

        public TeleportContractExportException Error(MemberInfo member, string message) =>
            new(member, message);

        public TypeExpr ExportType(Type type, MemberInfo member, bool allowNullableWrapper = true)
        {
            ArgumentNullException.ThrowIfNull(type);

            if (type == typeof(void))
            {
                throw Error(member, "void is not a supported contract type");
            }

            if (type == typeof(ValueTuple))
            {
                return new TypeExpr.Tuple([]);
            }

            if (type.IsGenericParameter)
            {
                throw Error(member, $"open generic parameter `{type.Name}` is not supported in contracts");
            }

            if (TryGetNullableUnderlyingType(type, out var nullableInner))
            {
                if (!allowNullableWrapper)
                {
                    return ExportType(nullableInner, member);
                }

                if (_exportedTypes.TryGetValue(type, out var cachedNullable))
                {
                    return cachedNullable;
                }

                return Cache(type, new TypeExpr.Nullable(ExportType(nullableInner, member)));
            }

            if (_exportedTypes.TryGetValue(type, out var cached))
            {
                return cached;
            }

            if (TryGetPrimitive(type, out var primitive))
            {
                return Cache(type, new TypeExpr.Primitive(primitive));
            }

            if (TryGetSequenceElementType(type, out var elementType))
            {
                return Cache(type, new TypeExpr.List(ExportType(elementType, member)));
            }

            if (TryGetMapTypes(type, out var keyType, out var valueType))
            {
                return Cache(type, new TypeExpr.Map(ExportType(keyType, member), ExportType(valueType, member)));
            }

            if (TryGetTupleElements(type, out var elements))
            {
                return Cache(type, new TypeExpr.Tuple(elements.Select(element => ExportType(element, member)).ToArray()));
            }

            if (type.IsEnum)
            {
                EnsureNamedType(type, member);
                return Cache(type, new TypeExpr.Named(GetTypeName(type), []));
            }

            if (type.IsGenericType && !type.IsConstructedGenericType)
            {
                throw Error(member, $"open generic type `{type}` is not supported in contracts");
            }

            if (IsSupportedNamedType(type))
            {
                EnsureNamedType(type, member);
                return Cache(type, new TypeExpr.Named(GetTypeName(type), []));
            }

            throw Error(member, $"unsupported contract type `{type}`");
        }

        public (TypeExpr Output, TypeExpr Error) ExportReturnTypes(MethodInfo method)
        {
            var returnType = UnwrapTask(method.ReturnType);
            if (!returnType.IsGenericType || returnType.GetGenericTypeDefinition() != typeof(TeleportResult<,>))
            {
                throw Error(method, "return type must be `TeleportResult<TOutput, TError>` or `Task<TeleportResult<TOutput, TError>>`");
            }

            var args = returnType.GetGenericArguments();
            return (
                ExportType(args[0], method, allowNullableWrapper: true),
                ExportType(args[1], method, allowNullableWrapper: true)
            );
        }

        private static Type UnwrapTask(Type type)
        {
            if (!type.IsGenericType)
            {
                return type;
            }

            var def = type.GetGenericTypeDefinition();
            if (def == typeof(Task<>) || def == typeof(ValueTask<>))
            {
                return type.GetGenericArguments()[0];
            }

            return type;
        }

        private TypeExpr Cache(Type type, TypeExpr value)
        {
            _exportedTypes[type] = value;
            return value;
        }

        private void EnsureNamedType(Type type, MemberInfo member)
        {
            var name = GetTypeName(type);
            if (_namedTypeNames.TryGetValue(type, out var existing))
            {
                if (existing != name)
                {
                    throw Error(member, $"conflicting export name for type `{type}`");
                }
                return;
            }

            if (_namedTypeNames.Values.Any(existing => existing == name))
            {
                throw Error(member, $"duplicate exported type name `{name}`");
            }

            _namedTypeNames[type] = name;
            if (!_namedTypeInProgress.Add(type))
            {
                return;
            }

            try
            {
                _namedTypes.Add(BuildNamedType(type, member));
            }
            finally
            {
                _namedTypeInProgress.Remove(type);
            }
        }

        private NamedTypeContract BuildNamedType(Type type, MemberInfo member)
        {
            if (type.IsEnum)
            {
                return new NamedTypeContract
                {
                    Name = GetTypeName(type),
                    Docs = GetDoc(type),
                    Generics = [],
                    Kind = new NamedTypeKind.Enum(
                        Enum.GetNames(type)
                            .OrderBy(name => name, StringComparer.Ordinal)
                            .Select(name => new VariantContract
                            {
                                Name = name,
                                Docs = GetDoc(type.GetField(name)),
                                Fields = new FieldsContract.Unit(),
                            })
                            .ToArray()),
                };
            }

            var members = GetDataMembers(type)
                .OrderBy(memberInfo => memberInfo.MetadataToken)
                .ToArray();

            var fields = new List<NamedFieldContract>(members.Length);
            foreach (var dataMember in members)
            {
                var memberType = GetMemberType(dataMember);
                var nullable = IsNullable(dataMember);
                TypeExpr? ty = null;
                if (memberType is not null)
                {
                    ty = nullable
                        ? new TypeExpr.Nullable(ExportType(memberType, dataMember, allowNullableWrapper: false))
                        : ExportType(memberType, dataMember, allowNullableWrapper: true);
                }

                fields.Add(new NamedFieldContract
                {
                    Name = GetJsonMemberName(dataMember),
                    Docs = GetDoc(dataMember),
                    Optional = false,
                    Ty = ty,
                });
            }

            return new NamedTypeContract
            {
                Name = GetTypeName(type),
                Docs = GetDoc(type),
                Generics = [],
                Kind = fields.Count == 0
                    ? new NamedTypeKind.Struct(new FieldsContract.Unit())
                    : new NamedTypeKind.Struct(new FieldsContract.Named(fields)),
            };
        }

        private static IEnumerable<MemberInfo> GetDataMembers(Type type)
        {
            foreach (var property in type.GetProperties(BindingFlags.Public | BindingFlags.Instance))
            {
                if (property.GetIndexParameters().Length > 0)
                {
                    continue;
                }

                if (property.GetMethod is null || !property.GetMethod.IsPublic)
                {
                    continue;
                }

                if (HasJsonIgnore(property))
                {
                    continue;
                }

                yield return property;
            }

            foreach (var field in type.GetFields(BindingFlags.Public | BindingFlags.Instance))
            {
                if (field.IsStatic || HasJsonIgnore(field))
                {
                    continue;
                }

                yield return field;
            }
        }

        private bool IsNullable(MemberInfo member)
        {
            var nullability = member switch
            {
                PropertyInfo property => _nullability.Create(property),
                FieldInfo field => _nullability.Create(field),
                _ => throw new UnreachableException(),
            };

            return nullability.ReadState == NullabilityState.Nullable || Nullable.GetUnderlyingType(GetMemberType(member)!) is not null;
        }

        private static Type? GetMemberType(MemberInfo member) => member switch
        {
            PropertyInfo property => property.PropertyType,
            FieldInfo field => field.FieldType,
            _ => null,
        };

        private string GetJsonMemberName(MemberInfo member)
        {
            var name = member.GetCustomAttribute<JsonPropertyNameAttribute>()?.Name;
            if (!string.IsNullOrWhiteSpace(name))
            {
                return name!;
            }

            return NamingPolicy.ConvertName(member.Name);
        }

        private string GetTypeName(Type type)
        {
            if (_namedTypeNames.TryGetValue(type, out var existing))
            {
                return existing;
            }

            var name = type.Name;
            var tick = name.IndexOf('`');
            if (tick >= 0)
            {
                name = name[..tick];
            }

            name = name.Replace('+', '_');
            return name;
        }

        private static bool TryGetPrimitive(Type type, out PrimitiveType primitive)
        {
            primitive = default;
            if (type == typeof(byte))
            {
                primitive = PrimitiveType.u8;
                return true;
            }

            if (type == typeof(sbyte))
            {
                primitive = PrimitiveType.i8;
                return true;
            }

            if (type == typeof(short))
            {
                primitive = PrimitiveType.i16;
                return true;
            }

            if (type == typeof(ushort))
            {
                primitive = PrimitiveType.u16;
                return true;
            }

            if (type == typeof(int))
            {
                primitive = PrimitiveType.i32;
                return true;
            }

            if (type == typeof(uint))
            {
                primitive = PrimitiveType.u32;
                return true;
            }

            if (type == typeof(long))
            {
                primitive = PrimitiveType.i64;
                return true;
            }

            if (type == typeof(ulong))
            {
                primitive = PrimitiveType.u64;
                return true;
            }

            if (type == typeof(nint) || type == typeof(IntPtr))
            {
                primitive = PrimitiveType.isize;
                return true;
            }

            if (type == typeof(nuint) || type == typeof(UIntPtr))
            {
                primitive = PrimitiveType.usize;
                return true;
            }

            if (type == typeof(float))
            {
                primitive = PrimitiveType.f32;
                return true;
            }

            if (type == typeof(double))
            {
                primitive = PrimitiveType.f64;
                return true;
            }

            if (type == typeof(bool))
            {
                primitive = PrimitiveType.@bool;
                return true;
            }

            if (type == typeof(char))
            {
                primitive = PrimitiveType.@char;
                return true;
            }

            if (type == typeof(string))
            {
                primitive = PrimitiveType.@str;
                return true;
            }

            return false;
        }

        private static bool TryGetNullableUnderlyingType(Type type, out Type inner)
        {
            inner = Nullable.GetUnderlyingType(type)!;
            return inner is not null;
        }

        private static bool TryGetSequenceElementType(Type type, out Type elementType)
        {
            elementType = default!;
            if (type == typeof(byte[]))
            {
                elementType = typeof(byte);
                return true;
            }

            if (type.IsArray)
            {
                elementType = type.GetElementType()!;
                return true;
            }

            if (type.IsGenericType)
            {
                var definition = type.GetGenericTypeDefinition();
                if (definition == typeof(List<>) || definition == typeof(IReadOnlyList<>) || definition == typeof(IList<>) ||
                    definition == typeof(IEnumerable<>) || definition == typeof(ICollection<>) || definition == typeof(ISet<>) ||
                    definition == typeof(HashSet<>))
                {
                    elementType = type.GetGenericArguments()[0];
                    return true;
                }
            }

            var sequenceInterface = type
                .GetInterfaces()
                .FirstOrDefault(interfaceType => interfaceType.IsGenericType && interfaceType.GetGenericTypeDefinition() == typeof(IEnumerable<>));
            if (sequenceInterface is not null)
            {
                elementType = sequenceInterface.GetGenericArguments()[0];
                return true;
            }

            return false;
        }

        private static bool TryGetMapTypes(Type type, out Type keyType, out Type valueType)
        {
            keyType = default!;
            valueType = default!;

            if (type.IsGenericType)
            {
                var definition = type.GetGenericTypeDefinition();
                if (definition == typeof(Dictionary<,>) || definition == typeof(IDictionary<,>) || definition == typeof(IReadOnlyDictionary<,>))
                {
                    var args = type.GetGenericArguments();
                    keyType = args[0];
                    valueType = args[1];
                    return true;
                }
            }

            var dictionaryInterface = type
                .GetInterfaces()
                .FirstOrDefault(interfaceType => interfaceType.IsGenericType && interfaceType.GetGenericTypeDefinition() == typeof(IDictionary<,>));
            if (dictionaryInterface is not null)
            {
                var args = dictionaryInterface.GetGenericArguments();
                keyType = args[0];
                valueType = args[1];
                return true;
            }

            return false;
        }

        private static bool TryGetTupleElements(Type type, out Type[] elements)
        {
            elements = [];
            if (!type.IsValueType || !type.IsGenericType)
            {
                return false;
            }

            if (type.FullName is null || !type.FullName.StartsWith("System.ValueTuple", StringComparison.Ordinal))
            {
                return false;
            }

            elements = type.GetGenericArguments();
            return true;
        }

        private static bool IsSupportedNamedType(Type type) =>
            (type.IsClass || type.IsValueType) &&
            !type.IsPrimitive &&
            type.Namespace is not null &&
            !type.Namespace.StartsWith("System", StringComparison.Ordinal) &&
            !type.Namespace.StartsWith("Microsoft", StringComparison.Ordinal);

        private static bool HasJsonIgnore(MemberInfo member) =>
            member.GetCustomAttribute<JsonIgnoreAttribute>() is not null;

    }
}

public sealed class TeleportContractExportException : InvalidOperationException
{
    public TeleportContractExportException(MemberInfo member, string message)
        : base($"{member.DeclaringType?.FullName ?? member.Name}.{member.Name}: {message}")
    {
        Member = member;
    }

    public MemberInfo Member { get; }
}
