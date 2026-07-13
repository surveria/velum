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

var setters = new Date(0);
if (setters.setFullYear(2001, 1, 3) !== Date.UTC(2001, 1, 3)) {
  throw new Test262Error("Date.prototype.setFullYear mismatch");
}
if (setters.setMonth(2, 4) !== Date.UTC(2001, 2, 4)) {
  throw new Test262Error("Date.prototype.setMonth mismatch");
}
if (setters.setDate(5) !== Date.UTC(2001, 2, 5)) {
  throw new Test262Error("Date.prototype.setDate mismatch");
}
if (setters.setHours(6, 7, 8, 9) !== Date.UTC(2001, 2, 5, 6, 7, 8, 9)) {
  throw new Test262Error("Date.prototype.setHours mismatch");
}
if (setters.setMinutes(10, 11, 12) !== Date.UTC(2001, 2, 5, 6, 10, 11, 12)) {
  throw new Test262Error("Date.prototype.setMinutes mismatch");
}
if (setters.setSeconds(13, 14) !== Date.UTC(2001, 2, 5, 6, 10, 13, 14)) {
  throw new Test262Error("Date.prototype.setSeconds mismatch");
}
if (setters.setMilliseconds(15) !== Date.UTC(2001, 2, 5, 6, 10, 13, 15)) {
  throw new Test262Error("Date.prototype.setMilliseconds mismatch");
}
if (setters.toISOString() !== "2001-03-05T06:10:13.015Z") {
  throw new Test262Error("Date setter final ISO mismatch");
}

var utcSetters = new Date(0);
utcSetters.setUTCFullYear(2022, 11, 31);
utcSetters.setUTCMonth(0, 2);
utcSetters.setUTCDate(3);
utcSetters.setUTCHours(4, 5, 6, 7);
utcSetters.setUTCMinutes(8, 9, 10);
utcSetters.setUTCSeconds(11, 12);
utcSetters.setUTCMilliseconds(13);
if (utcSetters.toISOString() !== "2022-01-03T04:08:11.013Z") {
  throw new Test262Error("UTC Date setter family mismatch");
}

var invalidFullYear = new Date(NaN);
if (invalidFullYear.setFullYear(2020) !== Date.UTC(2020, 0, 1)) {
  throw new Test262Error("setFullYear must recover invalid dates");
}
var invalidMonth = new Date(NaN);
var invalidMonthResult = invalidMonth.setMonth(1);
if (invalidMonthResult === invalidMonthResult || invalidMonth.getTime() === invalidMonth.getTime()) {
  throw new Test262Error("non-year setters must keep invalid dates invalid");
}
var invalidOffset = new Date(NaN).getTimezoneOffset();
if (new Date(0).getTimezoneOffset() !== 0 || invalidOffset === invalidOffset) {
  throw new Test262Error("Date.prototype.getTimezoneOffset mismatch");
}

var primitive = Date.prototype[Symbol.toPrimitive];
if (primitive.name !== "[Symbol.toPrimitive]" || primitive.length !== 1) {
  throw new Test262Error("Date @@toPrimitive descriptor surface mismatch");
}
var primitiveDate = new Date(0);
if (primitive.call(primitiveDate, "default") !== primitiveDate.toString()) {
  throw new Test262Error("Date @@toPrimitive default hint mismatch");
}
if (primitive.call(primitiveDate, "string") !== primitiveDate.toString()) {
  throw new Test262Error("Date @@toPrimitive string hint mismatch");
}
if (primitive.call(primitiveDate, "number") !== 0) {
  throw new Test262Error("Date @@toPrimitive number hint mismatch");
}
var order = "";
var ordinary = {
  toString() { order += "s"; return "ordinary"; },
  valueOf() { order += "v"; return 7; }
};
if (primitive.call(ordinary, "default") !== "ordinary" || order !== "s") {
  throw new Test262Error("Date @@toPrimitive ordinary default order mismatch");
}
order = "";
if (primitive.call(ordinary, "number") !== 7 || order !== "v") {
  throw new Test262Error("Date @@toPrimitive ordinary number order mismatch");
}
var invalidHintError = "";
try {
  primitive.call(primitiveDate, "bad");
} catch (error) {
  invalidHintError = error.name;
}
if (invalidHintError !== "TypeError") {
  throw new Test262Error("Date @@toPrimitive invalid hint must throw TypeError");
}

if (new Date(0).getYear() !== 70) {
  throw new Test262Error("Date.prototype.getYear mismatch");
}
var setYearDate = new Date(0);
if (setYearDate.setYear(99) !== Date.UTC(1999, 0, 1) || setYearDate.toISOString() !== "1999-01-01T00:00:00.000Z") {
  throw new Test262Error("Date.prototype.setYear mismatch");
}
var setYearInvalid = new Date(NaN);
var setYearInvalidResult = setYearInvalid.setYear();
if (setYearInvalidResult === setYearInvalidResult || setYearInvalid.getTime() === setYearInvalid.getTime()) {
  throw new Test262Error("Date.prototype.setYear invalid argument mismatch");
}
if (Date.prototype.toGMTString !== Date.prototype.toUTCString) {
  throw new Test262Error("Date.prototype.toGMTString alias mismatch");
}
if (setYearDate.toLocaleString() !== "1/1/1999, 12:00:00 AM") {
  throw new Test262Error("Date.prototype.toLocaleString Intl mismatch");
}
if (setYearDate.toLocaleDateString() !== "1/1/1999") {
  throw new Test262Error("Date.prototype.toLocaleDateString Intl mismatch");
}
if (setYearDate.toLocaleTimeString() !== "12:00:00 AM") {
  throw new Test262Error("Date.prototype.toLocaleTimeString Intl mismatch");
}
if (Date.prototype.toLocaleString.length !== 0 || Date.prototype.toLocaleDateString.length !== 0 || Date.prototype.toLocaleTimeString.length !== 0) {
  throw new Test262Error("Date.prototype.toLocale* length mismatch");
}

42;
