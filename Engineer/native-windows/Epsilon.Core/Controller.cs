using System.Collections.Frozen;
using System.Text.Json;

namespace Epsilon.Core;

public enum Profile { Power, Coolant }
public static class Systems
{
    public static readonly IReadOnlyList<string> Ids = Array.AsReadOnly(new[] { "reactor", "beamweapons", "missilesystem", "maneuver", "impulse", "warp", "frontshield", "rearshield" });
    public static readonly IReadOnlyList<string> Labels = Array.AsReadOnly(new[] { "Reactor", "Beam Weapons", "Missile System", "Maneuvering", "Impulse Engines", "Warp Drive", "Front Shields", "Rear Shields" });
}

public sealed record Configuration(string ShipName, IReadOnlyDictionary<string, int> Power, IReadOnlyDictionary<string, int> Coolant)
{
    public static Configuration Parse(string json)
    {
        using var document = JsonDocument.Parse(json);
        var root = document.RootElement;
        CheckDuplicates(root, "config");
        var version = Required(root, "version", "config");
        if (version.ValueKind != JsonValueKind.Number || !version.TryGetInt32(out int v) || v != 1)
            throw new FormatException("Configuration version must be 1.");
        var ship = Required(root, "shipName", "config");
        if (ship.ValueKind != JsonValueKind.String || string.IsNullOrWhiteSpace(ship.GetString()) || ship.GetString()!.Length > 60)
            throw new FormatException("shipName must contain 1–60 characters.");
        var defaults = Required(root, "defaults", "config");
        return new Configuration(ship.GetString()!, ReadProfile(defaults, "power"), ReadProfile(defaults, "coolant"));
    }
    static JsonElement Required(JsonElement parent, string key, string path)
    {
        if (parent.ValueKind != JsonValueKind.Object || !parent.TryGetProperty(key, out var result))
            throw new FormatException($"Missing {path}.{key}.");
        return result;
    }
    static void CheckDuplicates(JsonElement element, string path)
    {
        if (element.ValueKind != JsonValueKind.Object) return;
        var seen = new HashSet<string>();
        foreach (var property in element.EnumerateObject())
        {
            if (!seen.Add(property.Name)) throw new FormatException($"Duplicate property {path}.{property.Name}.");
            CheckDuplicates(property.Value, path + "." + property.Name);
        }
    }
    static IReadOnlyDictionary<string, int> ReadProfile(JsonElement defaults, string name)
    {
        var bank = Required(defaults, name, "defaults");
        return Systems.Ids.ToFrozenDictionary(id => id, id =>
        {
            var value = Required(bank, id, "defaults." + name);
            if (value.ValueKind != JsonValueKind.Number || !value.TryGetInt32(out int number) || number is < 0 or > 300)
                throw new FormatException($"defaults.{name}.{id} must be a whole number from 0 to 300.");
            return number;
        });
    }
}

public sealed record CoolantAllocation(int TotalWeight, double RulerMaximum, IReadOnlyDictionary<string, double> Percentages);
public static class Rules
{
    public static int Position(double value)
    {
        if (!double.IsFinite(value)) throw new ArgumentOutOfRangeException(nameof(value));
        return (int)Math.Clamp(Math.Round(value, MidpointRounding.AwayFromZero), 0, 300);
    }
    public static int Snap(double value)
    {
        int level = Position(value);
        foreach (int stop in new[] { 100, 150, 200, 250, 300 })
            if (Math.Abs(level - stop) <= 3) return stop;
        int nearest = (int)Math.Round(level / 5.0, MidpointRounding.AwayFromZero) * 5;
        return Math.Abs(level - nearest) <= 1 ? nearest : level;
    }
    public static CoolantAllocation Allocate(IReadOnlyDictionary<string, int> weights)
    {
        int total = Systems.Ids.Sum(id => weights[id]);
        double denominator = Math.Max(300, total);
        return new(total, 300 / denominator * 100, Systems.Ids.ToFrozenDictionary(id => id, id => weights[id] / denominator * 100));
    }
}

// Snapshots retain raw weights AND calculated pool percentages as different fields.
// A future HTTP adapter consumes this immutable snapshot off the UI thread.
public sealed record ControllerSnapshot(IReadOnlyDictionary<string, int> PowerPercentages,
    IReadOnlyDictionary<string, int> CoolantWeights, CoolantAllocation Coolant);
public interface IControllerAdapter { ValueTask ApplyAsync(ControllerSnapshot snapshot, CancellationToken cancellationToken); }
public sealed class LocalAdapter : IControllerAdapter
{
    public ControllerSnapshot? LastRequested { get; private set; }
    public ValueTask ApplyAsync(ControllerSnapshot snapshot, CancellationToken cancellationToken)
    { LastRequested = snapshot; return ValueTask.CompletedTask; }
}

public sealed class Controller
{
    readonly Configuration config;
    readonly Dictionary<string, int> power;
    readonly Dictionary<string, int> coolant;
    public Profile Active { get; private set; }
    public string ShipName => config.ShipName;
    public event Action? Changed;
    public Controller(Configuration configuration)
    {
        config = configuration;
        power = new(config.Power); coolant = new(config.Coolant);
    }
    Dictionary<string, int> Bank => Active == Profile.Power ? power : coolant;
    public int Get(string id) => Bank[id];
    public void Switch() { Active = Active == Profile.Power ? Profile.Coolant : Profile.Power; Changed?.Invoke(); }
    public void Set(string id, double value)
    {
        if (!Bank.ContainsKey(id)) throw new ArgumentException("Unknown engineering system.", nameof(id));
        int position = Rules.Position(value);
        if (Bank[id] == position) return;
        Bank[id] = position; Changed?.Invoke();
    }
    public void Commit(string id) => Set(id, Rules.Snap(Get(id)));
    public void Reset()
    {
        var defaults = Active == Profile.Power ? config.Power : config.Coolant;
        foreach (string id in Systems.Ids) Bank[id] = defaults[id];
        Changed?.Invoke();
    }
    public ControllerSnapshot Snapshot()
    {
        var weights = coolant.ToFrozenDictionary();
        return new(power.ToFrozenDictionary(), weights, Rules.Allocate(weights));
    }
}

// Exact piecewise-linear motion, shared by rendering and tests.
public readonly record struct MotionPoint(double Milliseconds, double Value);
public static class Motion
{
    public static IReadOnlyList<MotionPoint> Points(double start, double target)
    {
        double delta = target - start;
        double t = Math.Min(Math.Abs(delta), 300) / 300 * 170;
        return new[] { new MotionPoint(0, start), new(t, target + delta * .1),
            new(t + 24, target - delta * .01), new(t + 44, target + delta * .001),
            new(t + 62, target - delta * .0001), new(t + 80, target) };
    }
    public static double Sample(IReadOnlyList<MotionPoint> points, double time)
    {
        for (int i = 1; i < points.Count; i++)
            if (time < points[i].Milliseconds)
            {
                var a = points[i - 1]; var b = points[i];
                return a.Value + (b.Value - a.Value) * Math.Clamp((time - a.Milliseconds) / (b.Milliseconds - a.Milliseconds), 0, 1);
            }
        return points[^1].Value;
    }
    public static IReadOnlyList<MotionPoint> ClampedPoints(IReadOnlyList<MotionPoint> points)
    {
        var result = new List<MotionPoint> { new(0, Math.Clamp(points[0].Value, 0, 300)) };
        for (int i = 1; i < points.Count; i++)
        {
            var a = points[i - 1]; var b = points[i];
            foreach (double fraction in new[] { 0.0, 300.0 }.Select(edge => (edge - a.Value) / (b.Value - a.Value)).Where(f => f > 0 && f < 1).Order())
                result.Add(new(a.Milliseconds + fraction * (b.Milliseconds - a.Milliseconds), Math.Clamp(a.Value + fraction * (b.Value - a.Value), 0, 300)));
            result.Add(new(b.Milliseconds, Math.Clamp(b.Value, 0, 300)));
        }
        return result;
    }
}
