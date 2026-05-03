using System.Text.Json;
using System.Text.Json.Serialization;

namespace Teleport.Net;

public static class TeleportContractSchema
{
    public const string Version = "teleport.contract/v1";
}

public sealed record ContractBundle
{
    [JsonPropertyName("version")]
    public string Version { get; init; } = TeleportContractSchema.Version;

    [JsonPropertyName("procedures")]
    public List<ProcedureContract> Procedures { get; init; } = [];

    [JsonPropertyName("types")]
    public List<NamedTypeContract> Types { get; init; } = [];
}

public sealed record ProcedureContract
{
    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("namespace")]
    public required string Namespace { get; init; }

    [JsonPropertyName("method_name")]
    public required string MethodName { get; init; }

    [JsonPropertyName("procedure_kind")]
    public required ProcedureKind ProcedureKind { get; init; }

    [JsonPropertyName("http_method")]
    public required HttpMethod HttpMethod { get; init; }

    [JsonPropertyName("path")]
    public required string Path { get; init; }

    [JsonPropertyName("input_encoding")]
    public required InputEncoding InputEncoding { get; init; }

    [JsonPropertyName("auth_mode")]
    public required AuthMode AuthMode { get; init; }

    [JsonPropertyName("doc")]
    public required string Doc { get; init; }

    [JsonPropertyName("input_type")]
    public required TypeExpr InputType { get; init; }

    [JsonPropertyName("output_type")]
    public required TypeExpr OutputType { get; init; }

    [JsonPropertyName("error_type")]
    public required TypeExpr ErrorType { get; init; }
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum ProcedureKind
{
    Query,
    Command,
    Form,
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum HttpMethod
{
    Get,
    Post,
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum InputEncoding
{
    None,
    QueryString,
    JsonBody,
    FormBody,
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum AuthMode
{
    None,
    Required,
    Optional,
}

[JsonConverter(typeof(TypeExprJsonConverter))]
public abstract record TypeExpr
{
    public sealed record Primitive(PrimitiveType Value) : TypeExpr;

    public sealed record List(TypeExpr Element) : TypeExpr;

    public sealed record Map(TypeExpr Key, TypeExpr Value) : TypeExpr;

    public sealed record Tuple(IReadOnlyList<TypeExpr> Elements) : TypeExpr;

    public sealed record Nullable(TypeExpr Inner) : TypeExpr;

    public sealed record Named(string Name, IReadOnlyList<TypeExpr> Generics) : TypeExpr;

    public sealed record Generic(string Name) : TypeExpr;

    public sealed record Opaque(string Name) : TypeExpr;
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum PrimitiveType
{
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f16,
    f32,
    f64,
    f128,
    @bool,
    @char,
    @str,
}

public sealed record NamedTypeContract
{
    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("docs")]
    public required string Docs { get; init; }

    [JsonPropertyName("generics")]
    public required List<string> Generics { get; init; }

    [JsonPropertyName("kind")]
    public required NamedTypeKind Kind { get; init; }
}

[JsonConverter(typeof(NamedTypeKindJsonConverter))]
public abstract record NamedTypeKind
{
    public sealed record Struct(FieldsContract Fields) : NamedTypeKind;

    public sealed record Enum(IReadOnlyList<VariantContract> Variants) : NamedTypeKind;

    public sealed record Alias(TypeExpr Value) : NamedTypeKind;
}

[JsonConverter(typeof(FieldsContractJsonConverter))]
public abstract record FieldsContract
{
    public sealed record Unit : FieldsContract;

    public sealed record Named(IReadOnlyList<NamedFieldContract> Fields) : FieldsContract;

    public sealed record Unnamed(IReadOnlyList<UnnamedFieldContract> Fields) : FieldsContract;
}

public sealed record NamedFieldContract
{
    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("docs")]
    public required string Docs { get; init; }

    [JsonPropertyName("optional")]
    public required bool Optional { get; init; }

    [JsonPropertyName("ty")]
    public required TypeExpr? Ty { get; init; }
}

public sealed record UnnamedFieldContract
{
    [JsonPropertyName("docs")]
    public required string Docs { get; init; }

    [JsonPropertyName("ty")]
    public required TypeExpr? Ty { get; init; }
}

public sealed record VariantContract
{
    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("docs")]
    public required string Docs { get; init; }

    [JsonPropertyName("fields")]
    public required FieldsContract Fields { get; init; }
}

public interface ITeleportAppError
{
    int StatusCode { get; }
}

[JsonConverter(typeof(AppErrorJsonConverterFactory))]
public abstract record AppError<TDetail> : ITeleportAppError
{
    public sealed record Unauthorized : AppError<TDetail>;

    public sealed record Forbidden : AppError<TDetail>;

    public sealed record NotFound : AppError<TDetail>;

    public sealed record BadRequest(string Message) : AppError<TDetail>;

    public sealed record Internal(string Message) : AppError<TDetail>;

    public sealed record RateLimited : AppError<TDetail>;

    public sealed record Detail(TDetail Value) : AppError<TDetail>;

    public int StatusCode => this switch
    {
        Unauthorized => 401,
        Forbidden => 403,
        NotFound => 404,
        BadRequest => 400,
        Internal => 500,
        RateLimited => 429,
        Detail => 422,
        _ => 500,
    };

    public static AppError<TDetail> UnauthorizedError() => new Unauthorized();

    public static AppError<TDetail> ForbiddenError() => new Forbidden();

    public static AppError<TDetail> NotFoundError() => new NotFound();

    public static AppError<TDetail> BadRequestError(string message) => new BadRequest(message);

    public static AppError<TDetail> InternalError(string message) => new Internal(message);

    public static AppError<TDetail> RateLimitedError() => new RateLimited();

    public static AppError<TDetail> DetailError(TDetail detail) => new Detail(detail);
}

public interface ITeleportResult
{
    bool IsSuccess { get; }
    object? Value { get; }
    ITeleportAppError? Error { get; }
}

public abstract record TeleportResult<TOutput, TError> : ITeleportResult
{
    public sealed record Success(TOutput Data) : TeleportResult<TOutput, TError>;

    public sealed record Failure(AppError<TError> Cause) : TeleportResult<TOutput, TError>;

    public bool IsSuccess => this is Success;

    object? ITeleportResult.Value => this is Success success ? success.Data : default;

    ITeleportAppError? ITeleportResult.Error => this is Failure failure ? failure.Cause : default;

    public static TeleportResult<TOutput, TError> Ok(TOutput value) => new Success(value);

    public static TeleportResult<TOutput, TError> Fail(AppError<TError> error) => new Failure(error);
}

internal sealed class TypeExprJsonConverter : JsonConverter<TypeExpr>
{
    public override TypeExpr? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        throw new NotSupportedException("Teleport.Net only writes contract bundles.");

    public override void Write(Utf8JsonWriter writer, TypeExpr value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case TypeExpr.Primitive primitive:
                writer.WritePropertyName("Primitive");
                JsonSerializer.Serialize(writer, primitive.Value, options);
                break;
            case TypeExpr.List list:
                writer.WritePropertyName("List");
                JsonSerializer.Serialize(writer, list.Element, options);
                break;
            case TypeExpr.Map map:
                writer.WritePropertyName("Map");
                writer.WriteStartObject();
                writer.WritePropertyName("key");
                JsonSerializer.Serialize(writer, map.Key, options);
                writer.WritePropertyName("value");
                JsonSerializer.Serialize(writer, map.Value, options);
                writer.WriteEndObject();
                break;
            case TypeExpr.Tuple tuple:
                writer.WritePropertyName("Tuple");
                JsonSerializer.Serialize(writer, tuple.Elements, options);
                break;
            case TypeExpr.Nullable nullable:
                writer.WritePropertyName("Nullable");
                JsonSerializer.Serialize(writer, nullable.Inner, options);
                break;
            case TypeExpr.Named named:
                writer.WritePropertyName("Named");
                writer.WriteStartObject();
                writer.WriteString("name", named.Name);
                writer.WritePropertyName("generics");
                JsonSerializer.Serialize(writer, named.Generics, options);
                writer.WriteEndObject();
                break;
            case TypeExpr.Generic generic:
                writer.WritePropertyName("Generic");
                writer.WriteStringValue(generic.Name);
                break;
            case TypeExpr.Opaque opaque:
                writer.WritePropertyName("Opaque");
                writer.WriteStringValue(opaque.Name);
                break;
            default:
                throw new NotSupportedException($"Unsupported type expression: {value.GetType().FullName}");
        }
        writer.WriteEndObject();
    }
}

internal sealed class NamedTypeKindJsonConverter : JsonConverter<NamedTypeKind>
{
    public override NamedTypeKind? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        throw new NotSupportedException("Teleport.Net only writes contract bundles.");

    public override void Write(Utf8JsonWriter writer, NamedTypeKind value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case NamedTypeKind.Struct @struct:
                writer.WritePropertyName("Struct");
                JsonSerializer.Serialize(writer, @struct.Fields, options);
                break;
            case NamedTypeKind.Enum @enum:
                writer.WritePropertyName("Enum");
                JsonSerializer.Serialize(writer, @enum.Variants, options);
                break;
            case NamedTypeKind.Alias alias:
                writer.WritePropertyName("Alias");
                JsonSerializer.Serialize(writer, alias.Value, options);
                break;
            default:
                throw new NotSupportedException($"Unsupported named type kind: {value.GetType().FullName}");
        }
        writer.WriteEndObject();
    }
}

internal sealed class FieldsContractJsonConverter : JsonConverter<FieldsContract>
{
    public override FieldsContract? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        throw new NotSupportedException("Teleport.Net only writes contract bundles.");

    public override void Write(Utf8JsonWriter writer, FieldsContract value, JsonSerializerOptions options)
    {
        switch (value)
        {
            case FieldsContract.Unit:
                writer.WriteStringValue("Unit");
                break;
            case FieldsContract.Named named:
                writer.WriteStartObject();
                writer.WritePropertyName("Named");
                JsonSerializer.Serialize(writer, named.Fields, options);
                writer.WriteEndObject();
                break;
            case FieldsContract.Unnamed unnamed:
                writer.WriteStartObject();
                writer.WritePropertyName("Unnamed");
                JsonSerializer.Serialize(writer, unnamed.Fields, options);
                writer.WriteEndObject();
                break;
            default:
                throw new NotSupportedException($"Unsupported fields shape: {value.GetType().FullName}");
        }
    }
}

internal sealed class AppErrorJsonConverterFactory : JsonConverterFactory
{
    public override bool CanConvert(Type typeToConvert) =>
        typeToConvert.IsGenericType && typeToConvert.GetGenericTypeDefinition() == typeof(AppError<>);

    public override JsonConverter CreateConverter(Type typeToConvert, JsonSerializerOptions options)
    {
        var detailType = typeToConvert.GetGenericArguments()[0];
        var converterType = typeof(AppErrorJsonConverter<>).MakeGenericType(detailType);
        return (JsonConverter)Activator.CreateInstance(converterType)!;
    }
}

internal sealed class AppErrorJsonConverter<TDetail> : JsonConverter<AppError<TDetail>>
{
    public override AppError<TDetail>? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        throw new NotSupportedException("Teleport.Net only writes AppError values.");

    public override void Write(Utf8JsonWriter writer, AppError<TDetail> value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        writer.WritePropertyName("type");
        switch (value)
        {
            case AppError<TDetail>.Unauthorized:
                writer.WriteStringValue("Unauthorized");
                break;
            case AppError<TDetail>.Forbidden:
                writer.WriteStringValue("Forbidden");
                break;
            case AppError<TDetail>.NotFound:
                writer.WriteStringValue("NotFound");
                break;
            case AppError<TDetail>.BadRequest badRequest:
                writer.WriteStringValue("BadRequest");
                writer.WriteString("message", badRequest.Message);
                break;
            case AppError<TDetail>.Internal internalError:
                writer.WriteStringValue("Internal");
                writer.WriteString("message", internalError.Message);
                break;
            case AppError<TDetail>.RateLimited:
                writer.WriteStringValue("RateLimited");
                break;
            case AppError<TDetail>.Detail detail:
                writer.WriteStringValue("Detail");
                writer.WritePropertyName("detail");
                JsonSerializer.Serialize(writer, detail.Value, options);
                break;
            default:
                throw new NotSupportedException($"Unsupported AppError variant: {value.GetType().FullName}");
        }
        writer.WriteEndObject();
    }
}
