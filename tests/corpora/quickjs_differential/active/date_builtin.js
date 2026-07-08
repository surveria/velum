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
