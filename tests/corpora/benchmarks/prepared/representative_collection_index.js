// Bounded string-key Map/Set mutation and final materialization.
var __velumBenchKeys;

function __velumBenchSetup() {
    __velumBenchKeys = [];
    for (var i = 0; i < 257; i++) __velumBenchKeys.push("key-" + i);
}

function __velumBenchRun() {
    var values = new Map(), selected = new Set();
    for (var i = 0; i < 8192; i++) {
        var index = (i * 17) % __velumBenchKeys.length;
        var key = __velumBenchKeys[index];
        values.set(key, (values.get(key) || 0) + i % 31 + 1);
        if (i % 7 === 0) values.delete(__velumBenchKeys[(index + 3) % __velumBenchKeys.length]);
        if (i % 3 === 0) selected.add(key);
        else selected.delete(key);
    }
    var weighted = 0, members = 0;
    Array.from(values.entries()).forEach(function (entry) {
        var index = Number(entry[0].slice(4));
        weighted += (index + 1) * entry[1];
        if (selected.has(entry[0])) members++;
    });
    return values.size + ":" + weighted + ":" + selected.size + ":" + members;
}

function __velumBenchVerify() {
    return "242:1896614:86:81";
}
