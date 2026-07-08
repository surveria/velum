let epoch = new Date(0);
let utc = Date.UTC(2020, 0, 2, 3, 4, 5, 6);
let fromUtc = new Date(utc);
let fromString = new Date("2020-01-02T03:04:05.006Z");
let invalid = new Date(NaN);
let mutable = new Date(0);

print(epoch.toISOString());
print(epoch.toUTCString());
print(epoch.getUTCFullYear(), epoch.getUTCMonth(), epoch.getUTCDate(), epoch.getUTCDay());
print(fromUtc.toISOString(), fromString.getTime() === utc);
print(invalid.toString(), invalid.toJSON(), invalid.getTime() === invalid.getTime());
print(mutable.setTime(1000), mutable.toISOString());
print(typeof Date(), typeof Date.now(), Date.name, Date.length, Date.now.length, Date.parse.length, Date.UTC.length);

let utcSetters = new Date(0);
let utcFullYear = utcSetters.setUTCFullYear(2022, 11, 31);
utcSetters.setUTCMonth(0, 2);
utcSetters.setUTCDate(3);
utcSetters.setUTCHours(4, 5, 6, 7);
utcSetters.setUTCMinutes(8, 9, 10);
utcSetters.setUTCSeconds(11, 12);
utcSetters.setUTCMilliseconds(13);
print(utcSetters.toISOString(), utcFullYear);

let invalidFullYear = new Date(NaN);
let invalidMonth = new Date(NaN);
let invalidMonthResult = invalidMonth.setUTCMonth(1);
print(
  invalidFullYear.setUTCFullYear(2020),
  invalidFullYear.toISOString(),
  invalidMonthResult === invalidMonthResult,
  invalidMonth.getTime() === invalidMonth.getTime()
);

let primitive = Date.prototype[Symbol.toPrimitive];
let primitiveDate = new Date(0);
let order = "";
let ordinary = {
  toString() { order += "s"; return "ordinary"; },
  valueOf() { order += "v"; return 7; }
};
let ordinaryDefault = primitive.call(ordinary, "default");
let defaultOrder = order;
order = "";
let ordinaryNumber = primitive.call(ordinary, "number");
print(
  primitive.name,
  primitive.length,
  primitive.call(primitiveDate, "default") === primitiveDate.toString(),
  primitive.call(primitiveDate, "string") === primitiveDate.toString(),
  primitive.call(primitiveDate, "number"),
  ordinaryDefault,
  defaultOrder,
  ordinaryNumber,
  order
);
