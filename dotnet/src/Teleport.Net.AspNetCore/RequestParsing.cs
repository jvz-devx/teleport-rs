using System.Globalization;
using System.Text.Json.Nodes;
using Microsoft.AspNetCore.Http;

namespace Teleport.Net.AspNetCore;

internal static class QueryTreeParser
{
    public static JsonNode Parse(IQueryCollection query)
    {
        var root = new JsonObject();
        foreach (var pair in query)
        {
            foreach (var value in pair.Value)
            {
                Insert(root, pair.Key, value ?? string.Empty);
            }
        }

        return root;
    }

    public static JsonNode Parse(IFormCollection form)
    {
        var root = new JsonObject();
        foreach (var pair in form)
        {
            foreach (var value in pair.Value)
            {
                Insert(root, pair.Key, value ?? string.Empty);
            }
        }

        return root;
    }

    private static void Insert(JsonObject root, string key, string value)
    {
        var segments = ParseSegments(key);
        if (segments.Count == 0)
        {
            return;
        }

        InsertNode(root, segments, 0, ToJsonNode(value));
    }

    private static void InsertNode(JsonNode node, IReadOnlyList<PathSegment> segments, int index, JsonNode? value)
    {
        var segment = segments[index];
        var last = index == segments.Count - 1;

        if (segment is PathSegment.Property property)
        {
            var obj = EnsureObject(node);
            if (last)
            {
                AddProperty(obj, property.Name, value);
                return;
            }

            var child = obj[property.Name];
            if (child is null)
            {
                child = CreateContainer(segments[index + 1]);
                obj[property.Name] = child;
            }

            InsertNode(child, segments, index + 1, value);
            return;
        }

        if (segment is PathSegment.Index idx)
        {
            var array = EnsureArray(node);
            while (array.Count <= idx.Value)
            {
                array.Add(null);
            }

            if (last)
            {
                array[idx.Value] = value;
                return;
            }

            var child = array[idx.Value];
            if (child is null)
            {
                child = CreateContainer(segments[index + 1]);
                array[idx.Value] = child;
            }

            InsertNode(child, segments, index + 1, value);
            return;
        }

        var append = EnsureArray(node);
        if (last)
        {
            append.Add(value);
            return;
        }

        var appended = CreateContainer(segments[index + 1]);
        append.Add(appended);
        InsertNode(appended, segments, index + 1, value);
    }

    private static JsonObject EnsureObject(JsonNode node)
    {
        if (node is JsonObject obj)
        {
            return obj;
        }

        throw new InvalidOperationException("expected object container while parsing query/form data");
    }

    private static JsonArray EnsureArray(JsonNode node)
    {
        if (node is JsonArray array)
        {
            return array;
        }

        throw new InvalidOperationException("expected array container while parsing query/form data");
    }

    private static JsonNode CreateContainer(PathSegment segment)
    {
        return segment is PathSegment.Property ? new JsonObject() : new JsonArray();
    }

    private static void AddProperty(JsonObject obj, string name, JsonNode? value)
    {
        if (!obj.TryGetPropertyValue(name, out var existing) || existing is null)
        {
            obj[name] = value;
            return;
        }

        if (existing is JsonArray array)
        {
            array.Add(value);
            return;
        }

        obj[name] = new JsonArray(existing, value);
    }

    private static JsonNode? ToJsonNode(string value)
    {
        if (string.Equals(value, "null", StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        return JsonValue.Create(value);
    }

    private static List<PathSegment> ParseSegments(string key)
    {
        var segments = new List<PathSegment>();
        var current = new System.Text.StringBuilder();

        for (var i = 0; i < key.Length; i++)
        {
            var c = key[i];
            if (c == '[')
            {
                if (current.Length > 0)
                {
                    segments.Add(new PathSegment.Property(current.ToString()));
                    current.Clear();
                }

                var close = key.IndexOf(']', i + 1);
                if (close < 0)
                {
                    break;
                }

                var inside = key[(i + 1)..close];
                if (inside.Length == 0)
                {
                    segments.Add(PathSegment.AppendSegment.Instance);
                }
                else if (int.TryParse(inside, NumberStyles.Integer, CultureInfo.InvariantCulture, out var index))
                {
                    segments.Add(new PathSegment.Index(index));
                }
                else
                {
                    segments.Add(new PathSegment.Property(inside));
                }

                i = close;
                continue;
            }

            current.Append(c);
        }

        if (current.Length > 0)
        {
            segments.Add(new PathSegment.Property(current.ToString()));
        }

        return segments;
    }

    private abstract record PathSegment
    {
        public sealed record Property(string Name) : PathSegment;

        public sealed record Index(int Value) : PathSegment;

        public sealed record AppendSegment : PathSegment
        {
            public static readonly AppendSegment Instance = new();
        }
    }
}
