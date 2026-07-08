var epoch = new Date(0);
if (epoch.getTime() !== 0 || epoch.getUTCFullYear() !== 1970 || epoch.getUTCMonth() !== 0) {
  throw new Test262Error("epoch mismatch");
}

var comp = new Date(2020, 5, 15, 10, 30, 45, 500);
if (comp.getFullYear() !== 2020 || comp.getMonth() !== 5 || comp.getDate() !== 15) {
  throw new Test262Error("component date mismatch");
}
if (comp.getHours() !== 10 || comp.getMinutes() !== 30 || comp.getSeconds() !== 45 || comp.getMilliseconds() !== 500) {
  throw new Test262Error("component time mismatch");
}

if (Date.UTC(2000, 0, 1) !== 946684800000) {
  throw new Test262Error("Date.UTC mismatch");
}
if (typeof Date.now() !== "number") {
  throw new Test262Error("Date.now mismatch");
}

var iso = "2020-01-02T03:04:05.006Z";
var parsed = new Date(Date.parse(iso));
if (parsed.toISOString() !== iso) {
  throw new Test262Error("ISO round trip mismatch");
}

var copy = new Date(parsed);
if (copy.getTime() !== parsed.getTime() || copy === parsed) {
  throw new Test262Error("copy form mismatch");
}

var bad = new Date(NaN);
if (bad.getTime() === bad.getTime()) {
  throw new Test262Error("invalid date must be NaN");
}

var mut = new Date(0);
if (mut.setTime(123456) !== 123456 || mut.getTime() !== 123456) {
  throw new Test262Error("setTime mismatch");
}

if (Object.getPrototypeOf(new Date(0)) !== Date.prototype || !(new Date(0) instanceof Date)) {
  throw new Test262Error("prototype identity mismatch");
}

42;
