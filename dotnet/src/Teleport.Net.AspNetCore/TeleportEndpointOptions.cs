using System.Reflection;

namespace Teleport.Net.AspNetCore;

public sealed class TeleportEndpointOptions
{
    private readonly List<Assembly> _assemblies = [];

    public bool IncludeManifestEndpoint { get; set; }

    public IReadOnlyList<Assembly> Assemblies => _assemblies;

    public TeleportEndpointOptions AddAssembly(Assembly assembly)
    {
        ArgumentNullException.ThrowIfNull(assembly);

        if (!_assemblies.Contains(assembly))
        {
            _assemblies.Add(assembly);
        }

        return this;
    }

    public TeleportEndpointOptions AddAssemblyContaining<T>()
    {
        return AddAssembly(typeof(T).Assembly);
    }
}
