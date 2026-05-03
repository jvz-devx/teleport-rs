using System.Security.Claims;
using System.Text.Json.Serialization;
using Microsoft.AspNetCore.Mvc;
using Teleport.Net;

namespace Teleport.Net.TestFixtures;

public sealed class FixtureAssemblyMarker;

[TeleportModule("users")]
public static class ExportUsersApi
{
    [TeleportQuery]
    public static TeleportResult<FixtureUser, FixtureError> GetUser(GetUserById input) =>
        TeleportResult<FixtureUser, FixtureError>.Ok(new FixtureUser(input.id, "Ada", null));
}

[TeleportModule("auth")]
public static class ExportAuthApi
{
    [TeleportQuery]
    [TeleportName("getProfile")]
    public static TeleportResult<FixtureUser, ValueTuple> GetProfile([TeleportAuth] ClaimsPrincipal _auth) =>
        TeleportResult<FixtureUser, ValueTuple>.Ok(new FixtureUser("1", "Ada", null));
}

[TeleportModule("test")]
public static class TestApi
{
    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> Search(SearchInput input)
    {
        var text = $"{input.filter.author_id}|{input.filter.include_hidden}|{string.Join(",", input.tags)}";
        return TeleportResult<string, ValueTuple>.Ok(text);
    }

    [TeleportCommand]
    public static TeleportResult<CreateThing, ValueTuple> Create(CreateThing input) =>
        TeleportResult<CreateThing, ValueTuple>.Ok(input);

    [TeleportForm]
    public static TeleportResult<CreateThing, ValueTuple> SubmitForm(CreateThing input) =>
        TeleportResult<CreateThing, ValueTuple>.Ok(input);

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> Secret([TeleportAuth] ClaimsPrincipal auth) =>
        TeleportResult<string, ValueTuple>.Ok(auth.FindFirst(ClaimTypes.NameIdentifier)!.Value);

    [TeleportQuery]
    public static TeleportResult<string, ValidationDetail> FailDetail() =>
        TeleportResult<string, ValidationDetail>.Fail(AppError<ValidationDetail>.DetailError(new ValidationDetail(true)));

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> WhoAmI([TeleportAuth] ClaimsPrincipal? auth) =>
        TeleportResult<string, ValueTuple>.Ok(auth?.FindFirst(ClaimTypes.NameIdentifier)?.Value ?? "anonymous");

    [TeleportQuery]
    public static async Task<TeleportResult<string, ValueTuple>> AsyncEcho(GetUserById input)
    {
        await Task.Yield();
        return TeleportResult<string, ValueTuple>.Ok(input.id);
    }

    [TeleportQuery]
    public static TeleportResult<ValueTuple, ValueTuple> Ping() =>
        TeleportResult<ValueTuple, ValueTuple>.Ok(default);

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> FailForbidden() =>
        TeleportResult<string, ValueTuple>.Fail(AppError<ValueTuple>.ForbiddenError());

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> FailNotFound() =>
        TeleportResult<string, ValueTuple>.Fail(AppError<ValueTuple>.NotFoundError());

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> FailRateLimited() =>
        TeleportResult<string, ValueTuple>.Fail(AppError<ValueTuple>.RateLimitedError());

    [TeleportQuery]
    public static TeleportResult<string, ValueTuple> Crash() =>
        throw new InvalidOperationException("boom");
}

public sealed record GetUserById(string id);

public sealed record FixtureError(bool user_not_found);

public sealed record FixtureUser(
    string id,
    string name,
    [property: JsonPropertyName("avatar_url")] string? avatarUrl);

public sealed record SearchInput(SearchFilter filter, string[] tags);

public sealed record SearchFilter(
    [property: JsonPropertyName("author_id")] string author_id,
    [property: JsonPropertyName("include_hidden")] bool include_hidden);

public sealed record CreateThing(string title, int count);

public sealed record ValidationDetail(bool invalid);

public sealed class FixtureDependency
{
    public string Prefix { get; init; } = "fixture";
}

[TeleportDoc("Detailed exporter and runtime test module")]
[TeleportModule("meta")]
public static class MetadataApi
{
    [TeleportQuery]
    [TeleportName("describeComplex")]
    [TeleportDoc("Exports docs, collections, nullability, and enums")]
    public static TeleportResult<ComplexEnvelope, FixtureErrorCode> DescribeComplex(
        ComplexInput input,
        [FromServices] FixtureDependency dependency) =>
        TeleportResult<ComplexEnvelope, FixtureErrorCode>.Ok(new ComplexEnvelope
        {
            Payload = input,
            Empty = new EmptyShape(),
            Bytes = [1, 2, 3],
            Pair = (dependency.Prefix.Length, input.Title),
        });

    [TeleportQuery]
    [TeleportDoc("Collects repeated query values into arrays")]
    public static TeleportResult<string, ValueTuple> JoinNames(RepeatedQueryInput input) =>
        TeleportResult<string, ValueTuple>.Ok(string.Join(",", input.names));

    [TeleportForm]
    [TeleportDoc("Binds indexed form payloads into arrays of objects")]
    public static TeleportResult<string, ValueTuple> SubmitStructuredForm(StructuredFormInput input) =>
        TeleportResult<string, ValueTuple>.Ok(
            string.Join("|", input.items.Select(item => $"{item.name}:{item.quantity}")));
}

[TeleportDoc("Complex exporter input")]
public sealed class ComplexInput
{
    [TeleportDoc("Custom-named title")]
    [JsonPropertyName("display_title")]
    public string Title { get; init; } = string.Empty;

    [TeleportDoc("Array of counts")]
    public int[] Numbers { get; init; } = [];

    [TeleportDoc("List of optional counters")]
    public CounterEntry[] Counts { get; init; } = [];

    [TeleportDoc("Optional nested child")]
    public ComplexChild? Child { get; init; }

    [JsonIgnore]
    public string Hidden { get; init; } = string.Empty;
}

[TeleportDoc("Nested child")]
public sealed class ComplexChild
{
    [TeleportDoc("Nickname")]
    [JsonPropertyName("nick_name")]
    public string NickName { get; init; } = string.Empty;
}

[TeleportDoc("Counter entry")]
public sealed class CounterEntry
{
    [TeleportDoc("Counter key")]
    public string Key { get; init; } = string.Empty;

    [TeleportDoc("Counter value")]
    public int? Value { get; init; }
}

[TeleportDoc("Complex exporter output")]
public sealed class ComplexEnvelope
{
    [TeleportDoc("Payload echo")]
    public required ComplexInput Payload { get; init; }

    [TeleportDoc("Empty marker")]
    public required EmptyShape Empty { get; init; }

    [TeleportDoc("Binary-ish bytes")]
    public required byte[] Bytes { get; init; }

    [TeleportDoc("Tuple pair")]
    public required (int, string) Pair { get; init; }
}

[TeleportDoc("Zero-field shape")]
public sealed class EmptyShape;

[TeleportDoc("Fixture error codes")]
public enum FixtureErrorCode
{
    [TeleportDoc("Retry later")]
    Retryable,

    [TeleportDoc("Stop retrying")]
    Fatal,
}

public enum InspectMode
{
    First,
    Second,
}

public sealed class RepeatedQueryInput
{
    public string[] names { get; init; } = [];
}

public sealed class StructuredFormInput
{
    public StructuredFormItem[] items { get; init; } = [];
}

public sealed class StructuredFormItem
{
    public string name { get; init; } = string.Empty;

    public int quantity { get; init; }
}
