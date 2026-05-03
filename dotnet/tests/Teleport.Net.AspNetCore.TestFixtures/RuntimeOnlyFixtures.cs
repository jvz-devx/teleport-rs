using System.Security.Claims;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Mvc;
using Teleport.Net;
using Teleport.Net.TestFixtures;

namespace Teleport.Net.AspNetCore.TestFixtures;

[TeleportModule("runtime")]
public static class RuntimeOnlyApi
{
    [TeleportQuery]
    public static TeleportResult<RuntimeInvocationSnapshot, ValueTuple> InspectContext(
        GetUserById input,
        HttpContext context,
        CancellationToken cancellationToken,
        [FromServices] FixtureDependency dependency,
        [TeleportAuth] ClaimsPrincipal? auth) =>
        TeleportResult<RuntimeInvocationSnapshot, ValueTuple>.Ok(new RuntimeInvocationSnapshot
        {
            id = input.id,
            method = context.Request.Method,
            path = context.Request.Path.Value ?? string.Empty,
            servicePrefix = dependency.Prefix,
            cancellable = cancellationToken.CanBeCanceled,
            user = auth?.FindFirst(ClaimTypes.NameIdentifier)?.Value ?? "anonymous",
        });
}

public sealed class RuntimeInvocationSnapshot
{
    public string id { get; init; } = string.Empty;

    public string method { get; init; } = string.Empty;

    public string path { get; init; } = string.Empty;

    public string servicePrefix { get; init; } = string.Empty;

    public bool cancellable { get; init; }

    public string user { get; init; } = string.Empty;
}
