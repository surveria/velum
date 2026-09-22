// Object identity keys, extra lookups and a different churn distribution.
var __velumBenchKeys;

function __velumBenchSetup() {
    __velumBenchKeys = [];
    for (var i = 0; i < 193; i++) __velumBenchKeys.push({ id: i, group: i % 9 });
}

function __velumBenchRun() {
    var values = new Map(), selected = new Set();
    for (var i = 0; i < 8192; i++) {
        var index = (i * 29) % __velumBenchKeys.length;
        var key = __velumBenchKeys[index];
        var previous = values.has(key) ? values.get(key) : key.group;
        values.set(key, previous + i % 23 + 1);
        if (i % 5 === 0) values.delete(__velumBenchKeys[(index + 7) % __velumBenchKeys.length]);
        if (i % 4 < 2) selected.add(key);
        else selected.delete(key);
    }
    var weighted = 0, members = 0;
    values.forEach(function (value, key) {
        weighted += (key.id + 1) * value;
        if (selected.has(key)) members++;
    });
    return values.size + ":" + weighted + ":" + selected.size + ":" + members;
}

function __velumBenchVerify() {
    return "165:591214:96:82";
}
