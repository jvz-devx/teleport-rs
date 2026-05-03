using System.Net;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Text;
using System.Text.Json.Nodes;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Teleport.Net;
using Teleport.Net.AspNetCore;
using Teleport.Net.AspNetCore.TestFixtures;
using Teleport.Net.TestFixtures;

namespace Teleport.Net.AspNetCore.Tests;

public class HttpBehaviorTests
{
    [Fact]
    public async Task Query_binding_handles_nested_objects_arrays_and_string_values()
    {
        await using var host = await TeleportTestHost.StartAsync();
        var response = await host.Client.GetAsync(
            "/rpc/test.search?filter[author_id]=1&filter[include_hidden]=true&tags[]=rust&tags[]=rpc");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        Assert.Equal("\"1|True|rust,rpc\"", await response.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task Command_json_round_trips()
    {
        await using var host = await TeleportTestHost.StartAsync();
        var response = await host.Client.PostAsJsonAsync("/rpc/test.create", new CreateThing("hello", 2));

        response.EnsureSuccessStatusCode();
        var payload = await response.Content.ReadFromJsonAsync<CreateThing>();
        Assert.NotNull(payload);
        Assert.Equal("hello", payload!.title);
        Assert.Equal(2, payload.count);
    }

    [Fact]
    public async Task Form_endpoint_accepts_urlencoded_payloads()
    {
        await using var host = await TeleportTestHost.StartAsync();
        var response = await host.Client.PostAsync(
            "/rpc/test.submitForm",
            new FormUrlEncodedContent(new Dictionary<string, string>
            {
                ["title"] = "hello",
                ["count"] = "3",
            }));

        response.EnsureSuccessStatusCode();
        var payload = await response.Content.ReadFromJsonAsync<CreateThing>();
        Assert.NotNull(payload);
        Assert.Equal("hello", payload!.title);
        Assert.Equal(3, payload.count);
    }

    [Fact]
    public async Task Form_endpoint_also_accepts_json_payloads()
    {
        await using var host = await TeleportTestHost.StartAsync();
        var response = await host.Client.PostAsJsonAsync("/rpc/test.submitForm", new CreateThing("json", 4));

        response.EnsureSuccessStatusCode();
        var payload = await response.Content.ReadFromJsonAsync<CreateThing>();
        Assert.NotNull(payload);
        Assert.Equal("json", payload!.title);
        Assert.Equal(4, payload.count);
    }

    [Fact]
    public async Task Missing_auth_returns_structured_401_and_authenticated_requests_succeed()
    {
        await using var host = await TeleportTestHost.StartAsync();

        var unauthorized = await host.Client.GetAsync("/rpc/test.secret");
        Assert.Equal(HttpStatusCode.Unauthorized, unauthorized.StatusCode);
        var unauthorizedJson = JsonNode.Parse(await unauthorized.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal("Unauthorized", unauthorizedJson["type"]!.GetValue<string>());

        var request = new HttpRequestMessage(System.Net.Http.HttpMethod.Get, "/rpc/test.secret");
        request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", "demo-token");
        var authorized = await host.Client.SendAsync(request);

        authorized.EnsureSuccessStatusCode();
        Assert.Equal("\"demo-user\"", await authorized.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task Optional_auth_supports_anonymous_and_authenticated_requests()
    {
        await using var host = await TeleportTestHost.StartAsync();

        var anonymous = await host.Client.GetAsync("/rpc/test.whoAmI");
        anonymous.EnsureSuccessStatusCode();
        Assert.Equal("\"anonymous\"", await anonymous.Content.ReadAsStringAsync());

        var request = new HttpRequestMessage(System.Net.Http.HttpMethod.Get, "/rpc/test.whoAmI");
        request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", "demo-token");
        var authenticated = await host.Client.SendAsync(request);

        authenticated.EnsureSuccessStatusCode();
        Assert.Equal("\"demo-user\"", await authenticated.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task Detail_errors_and_bad_payloads_map_to_expected_error_shapes()
    {
        await using var host = await TeleportTestHost.StartAsync();

        var detail = await host.Client.GetAsync("/rpc/test.failDetail");
        Assert.Equal((HttpStatusCode)422, detail.StatusCode);
        var detailJson = JsonNode.Parse(await detail.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal("Detail", detailJson["type"]!.GetValue<string>());
        Assert.True(detailJson["detail"]!["invalid"]!.GetValue<bool>());

        var badPayload = await host.Client.GetAsync("/rpc/test.search?filter[author_id]=1&filter[include_hidden]=not-a-bool");
        Assert.Equal(HttpStatusCode.BadRequest, badPayload.StatusCode);
        var badPayloadJson = JsonNode.Parse(await badPayload.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal("BadRequest", badPayloadJson["type"]!.GetValue<string>());
    }

    [Fact]
    public async Task Invalid_json_command_payloads_map_to_bad_request()
    {
        await using var host = await TeleportTestHost.StartAsync();
        using var request = new HttpRequestMessage(System.Net.Http.HttpMethod.Post, "/rpc/test.create")
        {
            Content = new StringContent(
                "{\"count\":\"oops\"}",
                Encoding.UTF8,
                System.Net.Http.Headers.MediaTypeHeaderValue.Parse("application/json")),
        };

        var response = await host.Client.SendAsync(request);

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        var json = JsonNode.Parse(await response.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal("BadRequest", json["type"]!.GetValue<string>());
    }

    [Fact]
    public async Task Async_and_unit_query_endpoints_serialize_correctly()
    {
        await using var host = await TeleportTestHost.StartAsync();

        var asyncEcho = await host.Client.GetAsync("/rpc/test.asyncEcho?id=abc");
        asyncEcho.EnsureSuccessStatusCode();
        Assert.Equal("\"abc\"", await asyncEcho.Content.ReadAsStringAsync());

        var ping = await host.Client.GetAsync("/rpc/test.ping");
        ping.EnsureSuccessStatusCode();
        Assert.Equal("null", await ping.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task Status_mapped_errors_and_unhandled_exceptions_return_expected_responses()
    {
        await using var host = await TeleportTestHost.StartAsync();

        await AssertErrorAsync(host.Client, "/rpc/test.failForbidden", HttpStatusCode.Forbidden, "Forbidden");
        await AssertErrorAsync(host.Client, "/rpc/test.failNotFound", HttpStatusCode.NotFound, "NotFound");
        await AssertErrorAsync(host.Client, "/rpc/test.failRateLimited", (HttpStatusCode)429, "RateLimited");
        await AssertErrorAsync(host.Client, "/rpc/test.crash", HttpStatusCode.InternalServerError, "Internal");
    }

    [Fact]
    public async Task Manifest_endpoint_lists_registered_procedures()
    {
        await using var host = await TeleportTestHost.StartAsync();
        var response = await host.Client.GetAsync("/rpc/__manifest");

        response.EnsureSuccessStatusCode();
        var manifest = JsonNode.Parse(await response.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal("GET", manifest["procedures"]!["test.secret"]!["method"]!.GetValue<string>());
        Assert.Equal("/rpc/test.secret", manifest["procedures"]!["test.secret"]!["path"]!.GetValue<string>());
    }

    [Fact]
    public async Task Manifest_endpoint_is_absent_when_disabled()
    {
        await using var host = await TeleportTestHost.StartAsync(includeManifestEndpoint: false);

        var response = await host.Client.GetAsync("/rpc/__manifest");

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
    }

    [Fact]
    public async Task Wrong_http_method_returns_method_not_allowed()
    {
        await using var host = await TeleportTestHost.StartAsync();

        var response = await host.Client.GetAsync("/rpc/test.create");

        Assert.Equal(HttpStatusCode.MethodNotAllowed, response.StatusCode);
    }

    [Fact]
    public async Task Query_binding_supports_services_context_cancellation_and_optional_auth_parameters()
    {
        await using var host = await TeleportTestHost.StartAsync();

        var response = await host.Client.GetFromJsonAsync<RuntimeInvocationSnapshot>(
            "/rpc/runtime.inspectContext?id=abc");

        Assert.NotNull(response);
        Assert.Equal("abc", response!.id);
        Assert.Equal("GET", response.method);
        Assert.Equal("/rpc/runtime.inspectContext", response.path);
        Assert.Equal("fixture-service", response.servicePrefix);
        Assert.True(response.cancellable);
        Assert.Equal("anonymous", response.user);
    }

    [Fact]
    public async Task Form_binding_supports_indexed_object_arrays()
    {
        await using var host = await TeleportTestHost.StartAsync();
        var response = await host.Client.PostAsync(
            "/rpc/meta.submitStructuredForm",
            new FormUrlEncodedContent(new Dictionary<string, string>
            {
                ["items[0][name]"] = "apple",
                ["items[0][quantity]"] = "2",
                ["items[1][name]"] = "pear",
                ["items[1][quantity]"] = "5",
            }));

        response.EnsureSuccessStatusCode();
        Assert.Equal("\"apple:2|pear:5\"", await response.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task Missing_service_parameters_return_structured_internal_errors()
    {
        await using var host = await TeleportTestHost.StartAsync(registerFixtureDependency: false);

        var response = await host.Client.GetAsync("/rpc/runtime.inspectContext?id=abc");

        Assert.Equal(HttpStatusCode.InternalServerError, response.StatusCode);
        var json = JsonNode.Parse(await response.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal("Internal", json["type"]!.GetValue<string>());
    }

    [Fact]
    public void Exporter_rejects_primitive_query_inputs()
    {
        var ex = Assert.Throws<TeleportContractExportException>(() =>
            TeleportContractExporter.Build(typeof(BrokenQueryApi).Assembly));

        Assert.Contains("query inputs must be struct/class wrappers", ex.Message);
    }

    private static async Task AssertErrorAsync(
        HttpClient client,
        string path,
        HttpStatusCode statusCode,
        string errorType)
    {
        var response = await client.GetAsync(path);
        Assert.Equal(statusCode, response.StatusCode);

        var json = JsonNode.Parse(await response.Content.ReadAsStringAsync())!.AsObject();
        Assert.Equal(errorType, json["type"]!.GetValue<string>());
    }
}

internal sealed class TeleportTestHost : IAsyncDisposable
{
    private readonly WebApplication _app;

    private TeleportTestHost(WebApplication app, HttpClient client)
    {
        _app = app;
        Client = client;
    }

    public HttpClient Client { get; }

    public static async Task<TeleportTestHost> StartAsync(
        bool includeManifestEndpoint = true,
        bool registerFixtureDependency = true)
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        builder.Services.AddSingleton(new TestState());
        if (registerFixtureDependency)
        {
            builder.Services.AddSingleton(new FixtureDependency { Prefix = "fixture-service" });
        }
        builder.Services.AddTeleport();

        var app = builder.Build();
        app.Use(async (context, next) =>
        {
            if (context.Request.Headers.TryGetValue("Authorization", out var authorization) &&
                authorization.ToString() == "Bearer demo-token")
            {
                context.User = new ClaimsPrincipal(new ClaimsIdentity(
                [
                    new Claim(ClaimTypes.NameIdentifier, "demo-user"),
                ], "test"));
            }

            await next();
        });

        app.MapTeleportEndpoints(options =>
        {
            options.IncludeManifestEndpoint = includeManifestEndpoint;
            options.AddAssembly(typeof(FixtureAssemblyMarker).Assembly);
            options.AddAssembly(typeof(RuntimeOnlyApi).Assembly);
        });

        await app.StartAsync();
        return new TeleportTestHost(app, app.GetTestClient());
    }

    public async ValueTask DisposeAsync()
    {
        Client.Dispose();
        await _app.DisposeAsync();
    }
}

[TeleportModule("broken")]
public static class BrokenQueryApi
{
    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> Primitive(string id) =>
        TeleportResult<string, ValueTuple>.Ok(id);
}

public sealed record TestState;
