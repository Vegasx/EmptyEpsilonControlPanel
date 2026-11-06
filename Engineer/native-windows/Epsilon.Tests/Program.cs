using System.Text.Json.Nodes;
using Epsilon.Core;

int assertions = 0, groups = 0;
void Check(bool condition, string message) { assertions++; if (!condition) throw new Exception(message); }
void Equal(double actual, double expected, string message) => Check(Math.Abs(actual - expected) < 1e-9, $"{message}: expected {expected}, got {actual}");
void Reject(Action action, string message) { bool rejected = false; try { action(); } catch { rejected = true; } Check(rejected, message); }
void Test(string name, Action test) { test(); groups++; Console.WriteLine("PASS " + name); }
string configPath = args.Length == 0 ? Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../../config.json")) : args[0];
string json = File.ReadAllText(configPath);
Configuration config = Configuration.Parse(json);
Dictionary<string,int> Weights(params int[] values) => Systems.Ids.Select((id,index) => (id,index)).ToDictionary(x => x.id,x => x.index < values.Length ? values[x.index] : 0);

Test("snap boundaries and priority exhaustively, every integer 0–300", () =>
{
    for (int value = 0; value <= 300; value++)
    {
        int expected = value;
        int? major = new[] {100,150,200,250,300}.Cast<int?>().FirstOrDefault(stop => Math.Abs(value - stop!.Value) <= 3);
        if (major.HasValue) expected = major.Value;
        else { int five = (int)Math.Round(value / 5.0,MidpointRounding.AwayFromZero)*5; if (Math.Abs(five-value)<=1) expected=five; }
        Equal(Rules.Snap(value),expected,"snap " + value);
        Equal(Rules.Snap(Rules.Snap(value)),expected,"idempotence " + value);
    }
    foreach (var (value,expected) in new[] {(4,5),(6,5),(9,10),(11,10),(96,95),(97,100),(103,100),(104,105),(146,145),(147,150),(153,150),(154,155),(296,295),(297,300)}) Equal(Rules.Snap(value),expected,"explicit boundary");
    Equal(Rules.Snap(-10),0,"lower bound"); Equal(Rules.Snap(310),300,"upper bound");
    Reject(()=>Rules.Snap(double.NaN),"NaN rejected"); Reject(()=>Rules.Snap(double.PositiveInfinity),"infinity rejected");
});
Test("coolant examples, precision, capped ruler and unchanged weights", () =>
{
    foreach (var (values, shares, maximum) in new[] {
        (new[]{0},new[]{0.0},100.0), (new[]{100},new[]{100.0/3},100.0),
        (new[]{90,60},new[]{30.0,20.0},100.0), (new[]{300},new[]{100.0},100.0),
        (new[]{300,300},new[]{50.0,50.0},50.0), (new[]{300,300,300},new[]{100.0/3,100.0/3,100.0/3},100.0/3),
        (new[]{300,150,50},new[]{60.0,30.0,10.0},60.0) })
    {
        var weights=Weights(values); var before=weights.ToArray(); var allocation=Rules.Allocate(weights);
        Equal(allocation.RulerMaximum,maximum,"ruler");
        for(int i=0;i<shares.Length;i++) Equal(allocation.Percentages[Systems.Ids[i]],shares[i],"allocation");
        Check(weights.SequenceEqual(before),"allocation does not move any weight");
    }
    var random=new Random(41);
    for(int n=0;n<500;n++)
    {
        var raw=Weights(Enumerable.Range(0,8).Select(_=>random.Next(301)).ToArray()); var allocation=Rules.Allocate(raw);
        Check(allocation.RulerMaximum<=100,"ruler capped");
        Equal(allocation.Percentages.Values.Sum(),Math.Min(100,raw.Values.Sum()/3.0),"total budget");
    }
});
Test("configuration invalid forms, paths, version, ship name, duplicate keys", () =>
{
    foreach(string bad in new[]{"", "{", "null", "[]", "{}", json.Replace("\"version\": 1","\"version\": 2"), json.Replace("\"shipName\": \"ESS Endeavour\"","\"shipName\": \" \""), json.Replace("\"version\": 1","\"version\": 1, \"version\": 1")}) Reject(()=>Configuration.Parse(bad),"reject malformed config");
    foreach(string mode in new[]{"power","coolant"}) foreach(string id in Systems.Ids)
    {
        foreach(var bad in new JsonNode?[] { null, JsonValue.Create("100"), JsonValue.Create(true), JsonValue.Create(-1),JsonValue.Create(301),JsonValue.Create(99.5) })
        { var node=JsonNode.Parse(json)!; node["defaults"]![mode]![id]=bad?.DeepClone(); Reject(()=>Configuration.Parse(node.ToJsonString()),"invalid "+mode+"."+id); }
        var missing=JsonNode.Parse(json)!; ((JsonObject)missing["defaults"]![mode]!).Remove(id); Reject(()=>Configuration.Parse(missing.ToJsonString()),"missing "+id);
    }
    foreach(int length in new[]{1,60}) { var node=JsonNode.Parse(json)!; node["shipName"]=new string('x',length); Check(Configuration.Parse(node.ToJsonString()).ShipName.Length==length,"valid name boundary"); }
    var longName=JsonNode.Parse(json)!; longName["shipName"]=new string('x',61); Reject(()=>Configuration.Parse(longName.ToJsonString()),"long name");
});
Test("profile preservation and active-only reset with exact unsnapped defaults", () =>
{
    var node=JsonNode.Parse(json)!; node["defaults"]!["power"]!["reactor"]=99; node["defaults"]!["coolant"]!["reactor"]=149;
    var custom=Configuration.Parse(node.ToJsonString());
    foreach(Profile active in Enum.GetValues<Profile>())
    {
        var controller=new Controller(custom);
        foreach(var id in Systems.Ids) controller.Set(id,238);
        controller.Switch(); foreach(var id in Systems.Ids) controller.Set(id,191);
        if(controller.Active!=active) controller.Switch();
        var before=controller.Snapshot(); controller.Reset(); var after=controller.Snapshot();
        var restored=active==Profile.Power?after.PowerPercentages:after.CoolantWeights;
        var defaults=active==Profile.Power?custom.Power:custom.Coolant;
        Check(Systems.Ids.All(id=>restored[id]==defaults[id]),"exact defaults");
        Check(Systems.Ids.All(id=>active==Profile.Power?before.CoolantWeights[id]==after.CoolantWeights[id]:before.PowerPercentages[id]==after.PowerPercentages[id]),"inactive unchanged");
        controller.Set("reactor",100); controller.Set("reactor",101); Equal(controller.Get("reactor"),101,"keyboard can escape stop"); controller.Commit("reactor"); Equal(controller.Get("reactor"),100,"focus loss commits");
    }
});
Test("adapter snapshots immutable, complete and distinguish weights from shares", () =>
{
    var c=new Controller(config); c.Switch(); c.Set("reactor",300); c.Set("beamweapons",300);
    var s=c.Snapshot(); Equal(s.CoolantWeights["reactor"],300,"weight"); Equal(s.Coolant.Percentages["reactor"],50,"allocation");
    c.Set("reactor",0); Equal(s.CoolantWeights["reactor"],300,"snapshot isolated");
    Reject(()=>((IDictionary<string,int>)s.PowerPercentages)["reactor"]=0,"snapshot read-only");
});
Test("motion timing, true overshoot, continuity and clamped fill", () =>
{
    foreach(var (start,target) in new[]{(0.0,300.0),(300.0,0.0),(100.0,150.0),(120.0,100.0)})
    {
        var points=Motion.Points(start,target); double d=target-start;
        Equal(points[1].Value,target+d*.1,"overshoot"); Equal(points[2].Value,target-d*.01,"correction 1");
        Equal(points[3].Value,target+d*.001,"correction 2"); Equal(points[4].Value,target-d*.0001,"correction 3");
        Equal(points[^1].Value,target,"exact endpoint"); Equal(points[^1].Milliseconds,Math.Min(Math.Abs(d),300)/300*170+80,"duration");
        var clamped=Motion.ClampedPoints(points);
        for(double t=0;t<=250;t+=.25) Equal(Motion.Sample(clamped,t),Math.Clamp(Motion.Sample(points,t),0,300),"fill clamp");
        double visible=Motion.Sample(points,45); var reversed=Motion.Points(visible,start); Equal(Motion.Sample(reversed,0),visible,"interruption continuity");
    }
    Check(Motion.Points(0,10)[^1].Milliseconds<Motion.Points(0,300)[^1].Milliseconds,"short move finishes sooner");
});
Console.WriteLine($"PASS: {groups} groups; {assertions} assertions.");
