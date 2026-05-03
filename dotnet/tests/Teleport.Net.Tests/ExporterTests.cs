using Teleport.Net;
using Teleport.Net.TestFixtures;

namespace Teleport.Net.Tests;

public class ExporterTests
{
    [Fact]
    public void Exporter_builds_contract_bundle_for_wrapped_query_and_auth_procedures()
    {
        var bundle = TeleportContractExporter.Build(typeof(FixtureAssemblyMarker).Assembly);

        Assert.Equal(TeleportContractSchema.Version, bundle.Version);

        var getUser = Assert.Single(bundle.Procedures, proc => proc.Name == "users.getUser");
        Assert.Equal(ProcedureKind.Query, getUser.ProcedureKind);
        Assert.Equal(HttpMethod.Get, getUser.HttpMethod);
        Assert.Equal(InputEncoding.QueryString, getUser.InputEncoding);
        Assert.Equal(AuthMode.None, getUser.AuthMode);
        var getUserInput = Assert.IsType<TypeExpr.Named>(getUser.InputType);
        Assert.Equal("GetUserById", getUserInput.Name);

        var getProfile = Assert.Single(bundle.Procedures, proc => proc.Name == "auth.getProfile");
        Assert.Equal(AuthMode.Required, getProfile.AuthMode);
        Assert.Equal("/rpc/auth.getProfile", getProfile.Path);

        var whoAmI = Assert.Single(bundle.Procedures, proc => proc.Name == "test.whoAmI");
        Assert.Equal(AuthMode.Optional, whoAmI.AuthMode);
        Assert.Equal(InputEncoding.None, whoAmI.InputEncoding);
        Assert.IsType<TypeExpr.Tuple>(whoAmI.InputType);

        var asyncEcho = Assert.Single(bundle.Procedures, proc => proc.Name == "test.asyncEcho");
        Assert.Equal(ProcedureKind.Query, asyncEcho.ProcedureKind);
        Assert.Equal(HttpMethod.Get, asyncEcho.HttpMethod);
        var asyncEchoInput = Assert.IsType<TypeExpr.Named>(asyncEcho.InputType);
        Assert.Equal("GetUserById", asyncEchoInput.Name);

        var ping = Assert.Single(bundle.Procedures, proc => proc.Name == "test.ping");
        Assert.Equal(InputEncoding.None, ping.InputEncoding);
        Assert.IsType<TypeExpr.Tuple>(ping.InputType);
        Assert.IsType<TypeExpr.Tuple>(ping.OutputType);

        var userType = Assert.Single(bundle.Types, t => t.Name == "FixtureUser");
        var userFields = Assert.IsType<FieldsContract.Named>(Assert.IsType<NamedTypeKind.Struct>(userType.Kind).Fields);

        Assert.Contains(userFields.Fields, field => field.Name == "id" && field.Optional is false);
        Assert.Contains(userFields.Fields, field => field.Name == "avatar_url" && field.Optional is false);
    }

    [Fact]
    public void Exporter_includes_docs_custom_names_collections_and_enum_types()
    {
        var bundle = TeleportContractExporter.Build(typeof(FixtureAssemblyMarker).Assembly);

        var describeComplex = Assert.Single(bundle.Procedures, proc => proc.Name == "meta.describeComplex");
        Assert.Equal("Exports docs, collections, nullability, and enums", describeComplex.Doc);
        Assert.Equal("/rpc/meta.describeComplex", describeComplex.Path);
        Assert.Equal(InputEncoding.QueryString, describeComplex.InputEncoding);
        Assert.Equal(AuthMode.None, describeComplex.AuthMode);
        Assert.Equal("ComplexInput", Assert.IsType<TypeExpr.Named>(describeComplex.InputType).Name);
        Assert.Equal("ComplexEnvelope", Assert.IsType<TypeExpr.Named>(describeComplex.OutputType).Name);
        Assert.Equal("FixtureErrorCode", Assert.IsType<TypeExpr.Named>(describeComplex.ErrorType).Name);

        var complexInput = Assert.Single(bundle.Types, type => type.Name == "ComplexInput");
        Assert.Equal("Complex exporter input", complexInput.Docs);
        var complexFields = Assert.IsType<FieldsContract.Named>(Assert.IsType<NamedTypeKind.Struct>(complexInput.Kind).Fields);
        var title = Assert.Single(complexFields.Fields, field => field.Name == "display_title");
        Assert.Equal("Custom-named title", title.Docs);
        Assert.Equal(PrimitiveType.@str, Assert.IsType<TypeExpr.Primitive>(title.Ty).Value);

        var numbers = Assert.Single(complexFields.Fields, field => field.Name == "numbers");
        Assert.Equal(PrimitiveType.i32, Assert.IsType<TypeExpr.Primitive>(Assert.IsType<TypeExpr.List>(numbers.Ty).Element).Value);

        var counts = Assert.Single(complexFields.Fields, field => field.Name == "counts");
        Assert.Equal("CounterEntry", Assert.IsType<TypeExpr.Named>(Assert.IsType<TypeExpr.List>(counts.Ty).Element).Name);

        var child = Assert.Single(complexFields.Fields, field => field.Name == "child");
        Assert.Equal("ComplexChild", Assert.IsType<TypeExpr.Named>(Assert.IsType<TypeExpr.Nullable>(child.Ty).Inner).Name);
        Assert.DoesNotContain(complexFields.Fields, field => field.Name == "hidden");

        var counterEntry = Assert.Single(bundle.Types, type => type.Name == "CounterEntry");
        var counterFields = Assert.IsType<FieldsContract.Named>(Assert.IsType<NamedTypeKind.Struct>(counterEntry.Kind).Fields);
        var counterValue = Assert.Single(counterFields.Fields, field => field.Name == "value");
        Assert.Equal(PrimitiveType.i32, Assert.IsType<TypeExpr.Primitive>(Assert.IsType<TypeExpr.Nullable>(counterValue.Ty).Inner).Value);

        var complexChild = Assert.Single(bundle.Types, type => type.Name == "ComplexChild");
        var childFields = Assert.IsType<FieldsContract.Named>(Assert.IsType<NamedTypeKind.Struct>(complexChild.Kind).Fields);
        var nickname = Assert.Single(childFields.Fields, field => field.Name == "nick_name");
        Assert.Equal("Nickname", nickname.Docs);

        var complexEnvelope = Assert.Single(bundle.Types, type => type.Name == "ComplexEnvelope");
        var envelopeFields = Assert.IsType<FieldsContract.Named>(Assert.IsType<NamedTypeKind.Struct>(complexEnvelope.Kind).Fields);
        var bytes = Assert.Single(envelopeFields.Fields, field => field.Name == "bytes");
        Assert.Equal(PrimitiveType.u8, Assert.IsType<TypeExpr.Primitive>(Assert.IsType<TypeExpr.List>(bytes.Ty).Element).Value);
        var pair = Assert.Single(envelopeFields.Fields, field => field.Name == "pair");
        Assert.Equal(2, Assert.IsType<TypeExpr.Tuple>(pair.Ty).Elements.Count);

        var emptyShape = Assert.Single(bundle.Types, type => type.Name == "EmptyShape");
        Assert.IsType<FieldsContract.Unit>(Assert.IsType<NamedTypeKind.Struct>(emptyShape.Kind).Fields);

        var errorCode = Assert.Single(bundle.Types, type => type.Name == "FixtureErrorCode");
        var variants = Assert.IsType<NamedTypeKind.Enum>(errorCode.Kind).Variants;
        Assert.Collection(
            variants,
            variant =>
            {
                Assert.Equal("Fatal", variant.Name);
                Assert.Equal("Stop retrying", variant.Docs);
            },
            variant =>
            {
                Assert.Equal("Retryable", variant.Name);
                Assert.Equal("Retry later", variant.Docs);
            });
    }

    [Fact]
    public void Exporter_export_json_writes_indented_contract_document()
    {
        var json = TeleportContractExporter.ExportJson(typeof(FixtureAssemblyMarker).Assembly);

        Assert.Contains("\"version\": \"teleport.contract/v1\"", json);
        Assert.Contains("\"meta.describeComplex\"", json);
        Assert.Contains(Environment.NewLine + "  \"procedures\": [", json);
    }
}
