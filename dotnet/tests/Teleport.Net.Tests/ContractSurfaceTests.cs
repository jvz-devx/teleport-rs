using System.Text.Json;
using System.Text.Json.Nodes;
using Teleport.Net.TestFixtures;

namespace Teleport.Net.Tests;

public class ContractSurfaceTests
{
    [Fact]
    public void Attributes_results_and_errors_expose_expected_surface()
    {
        var module = new TeleportModuleAttribute("users");
        var name = new TeleportNameAttribute("lookup");
        var doc = new TeleportDocAttribute("Docs");

        Assert.Equal("users", module.Namespace);
        Assert.Equal("lookup", name.Name);
        Assert.Equal("Docs", doc.Text);

        var authUsage = Assert.IsType<AttributeUsageAttribute>(
            Attribute.GetCustomAttribute(typeof(TeleportAuthAttribute), typeof(AttributeUsageAttribute)));
        Assert.Equal(AttributeTargets.Parameter, authUsage.ValidOn);
        Assert.False(authUsage.AllowMultiple);

        var success = TeleportResult<string, ValidationDetail>.Ok("ok");
        var failure = TeleportResult<string, ValidationDetail>.Fail(
            AppError<ValidationDetail>.DetailError(new ValidationDetail(true)));

        Assert.True(success.IsSuccess);
        Assert.Equal("ok", ((ITeleportResult)success).Value);
        Assert.Null(((ITeleportResult)success).Error);

        Assert.False(failure.IsSuccess);
        Assert.Null(((ITeleportResult)failure).Value);
        var detailError = Assert.IsType<AppError<ValidationDetail>.Detail>(((ITeleportResult)failure).Error);
        Assert.True(detailError.Value.invalid);

        Assert.Equal(401, AppError<ValidationDetail>.UnauthorizedError().StatusCode);
        Assert.Equal(403, AppError<ValidationDetail>.ForbiddenError().StatusCode);
        Assert.Equal(404, AppError<ValidationDetail>.NotFoundError().StatusCode);
        Assert.Equal(400, AppError<ValidationDetail>.BadRequestError("bad").StatusCode);
        Assert.Equal(500, AppError<ValidationDetail>.InternalError("boom").StatusCode);
        Assert.Equal(429, AppError<ValidationDetail>.RateLimitedError().StatusCode);
        Assert.Equal(422, AppError<ValidationDetail>.DetailError(new ValidationDetail(false)).StatusCode);
    }

    [Fact]
    public void Contract_union_types_serialize_all_supported_variants()
    {
        Assert.Equal(
            "{\"Primitive\":\"i32\"}",
            JsonSerializer.Serialize<TypeExpr>(new TypeExpr.Primitive(PrimitiveType.i32)));
        Assert.Equal(
            "{\"List\":{\"Primitive\":\"str\"}}",
            JsonSerializer.Serialize<TypeExpr>(new TypeExpr.List(new TypeExpr.Primitive(PrimitiveType.@str))));
        Assert.Equal(
            "{\"Map\":{\"key\":{\"Primitive\":\"str\"},\"value\":{\"Primitive\":\"bool\"}}}",
            JsonSerializer.Serialize<TypeExpr>(
                new TypeExpr.Map(
                    new TypeExpr.Primitive(PrimitiveType.@str),
                    new TypeExpr.Primitive(PrimitiveType.@bool))));
        Assert.Equal(
            "{\"Tuple\":[{\"Primitive\":\"u8\"},{\"Named\":{\"name\":\"ComplexInput\",\"generics\":[]}}]}",
            JsonSerializer.Serialize<TypeExpr>(
                new TypeExpr.Tuple(
                [
                    new TypeExpr.Primitive(PrimitiveType.u8),
                    new TypeExpr.Named("ComplexInput", []),
                ])));
        Assert.Equal(
            "{\"Nullable\":{\"Primitive\":\"i64\"}}",
            JsonSerializer.Serialize<TypeExpr>(new TypeExpr.Nullable(new TypeExpr.Primitive(PrimitiveType.i64))));
        Assert.Equal(
            "{\"Generic\":\"TValue\"}",
            JsonSerializer.Serialize<TypeExpr>(new TypeExpr.Generic("TValue")));
        Assert.Equal(
            "{\"Opaque\":\"HttpContext\"}",
            JsonSerializer.Serialize<TypeExpr>(new TypeExpr.Opaque("HttpContext")));

        Assert.Equal(
            "{\"Struct\":\"Unit\"}",
            JsonSerializer.Serialize<NamedTypeKind>(new NamedTypeKind.Struct(new FieldsContract.Unit())));
        Assert.Equal(
            "{\"Enum\":[{\"name\":\"Fatal\",\"docs\":\"Stop retrying\",\"fields\":\"Unit\"}]}",
            JsonSerializer.Serialize<NamedTypeKind>(
                new NamedTypeKind.Enum(
                [
                    new VariantContract
                    {
                        Name = "Fatal",
                        Docs = "Stop retrying",
                        Fields = new FieldsContract.Unit(),
                    },
                ])));
        Assert.Equal(
            "{\"Alias\":{\"Primitive\":\"str\"}}",
            JsonSerializer.Serialize<NamedTypeKind>(
                new NamedTypeKind.Alias(new TypeExpr.Primitive(PrimitiveType.@str))));
        Assert.Equal(
            "{\"Unnamed\":[{\"docs\":\"inner\",\"ty\":{\"Primitive\":\"u32\"}}]}",
            JsonSerializer.Serialize<FieldsContract>(
                new FieldsContract.Unnamed(
                [
                    new UnnamedFieldContract
                    {
                        Docs = "inner",
                        Ty = new TypeExpr.Primitive(PrimitiveType.u32),
                    },
                ])));

        Assert.Throws<NotSupportedException>(() => JsonSerializer.Deserialize<TypeExpr>("{}"));
        Assert.Throws<NotSupportedException>(() => JsonSerializer.Deserialize<NamedTypeKind>("{}"));
        Assert.Throws<NotSupportedException>(() => JsonSerializer.Deserialize<FieldsContract>("\"Unit\""));
    }

    [Fact]
    public void App_error_json_serialization_matches_wire_shape()
    {
        Assert.Equal(
            "{\"type\":\"Unauthorized\"}",
            JsonSerializer.Serialize(AppError<ValidationDetail>.UnauthorizedError()));
        Assert.Equal(
            "{\"type\":\"Forbidden\"}",
            JsonSerializer.Serialize(AppError<ValidationDetail>.ForbiddenError()));
        Assert.Equal(
            "{\"type\":\"NotFound\"}",
            JsonSerializer.Serialize(AppError<ValidationDetail>.NotFoundError()));
        Assert.Equal(
            "{\"type\":\"BadRequest\",\"message\":\"bad payload\"}",
            JsonSerializer.Serialize(AppError<ValidationDetail>.BadRequestError("bad payload")));
        Assert.Equal(
            "{\"type\":\"Internal\",\"message\":\"internal\"}",
            JsonSerializer.Serialize(AppError<ValidationDetail>.InternalError("internal")));
        Assert.Equal(
            "{\"type\":\"RateLimited\"}",
            JsonSerializer.Serialize(AppError<ValidationDetail>.RateLimitedError()));

        var detailJson = JsonNode.Parse(JsonSerializer.Serialize(
            AppError<ValidationDetail>.DetailError(new ValidationDetail(true))))!.AsObject();
        Assert.Equal("Detail", detailJson["type"]!.GetValue<string>());
        Assert.True(detailJson["detail"]!["invalid"]!.GetValue<bool>());
    }
}
