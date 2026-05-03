using System.Text.Json;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.DependencyInjection;
using Teleport.Net;
using Teleport.Net.AspNetCore;
using Teleport.Net.Demo;

var exportOnly = args.Any(arg => arg == "--export-only");
var contractPath = DemoPaths.ContractPath;
var jsonOptions = new JsonSerializerOptions(JsonSerializerDefaults.Web);

await File.WriteAllTextAsync(
    contractPath,
    TeleportContractExporter.ExportJson(typeof(AuthApi).Assembly, jsonOptions));

if (exportOnly)
{
    Console.WriteLine($"Exported contract to {contractPath}");
    return;
}

var builder = Microsoft.AspNetCore.Builder.WebApplication.CreateBuilder(args);

builder.Services.AddSingleton(DemoState.Create());
builder.Services.AddTeleport();
builder.Services.AddCors(options =>
{
    options.AddPolicy("teleport-demo", policy =>
    {
        policy.WithOrigins("http://localhost:5173")
            .AllowAnyHeader()
            .AllowAnyMethod()
            .AllowCredentials();
    });
});

var app = builder.Build();

app.Use(async (context, next) =>
{
    var state = context.RequestServices.GetRequiredService<DemoState>();
    if (DemoAuth.TryAuthenticate(context, state, out var principal))
    {
        context.User = principal;
    }

    await next();
});

app.UseCors("teleport-demo");

app.MapTeleportEndpoints(options =>
{
    options.IncludeManifestEndpoint = true;
    options.AddAssemblyContaining<LoginRequest>();
});

app.MapGet("/", () => Results.Text("Teleport .NET demo running."));

Console.WriteLine($"Exported contract to {contractPath}");
Console.WriteLine("Server running on http://localhost:3000");
await app.RunAsync("http://0.0.0.0:3000");
