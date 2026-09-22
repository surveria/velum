// Parse event batches, validate nested payloads, project and serialize results.
var __velumBenchDocuments;

function __velumBenchSetup() {
    __velumBenchDocuments = [];
    for (var batch = 0; batch < 32; batch++) {
        var records = [];
        for (var j = 0; j < 16; j++) {
            var id = batch * 16 + j;
            records.push({ id: id, enabled: id % 4 !== 0, payload: { value: id % 97, name: "sensor-" + id } });
        }
        __velumBenchDocuments.push(JSON.stringify({ records: records, version: 1 }));
    }
}

function __velumBenchRun() {
    var count = 0, total = 0, bytes = 0;
    for (var repeat = 0; repeat < 4; repeat++) {
        for (var batch = 0; batch < __velumBenchDocuments.length; batch++) {
            var envelope = JSON.parse(__velumBenchDocuments[batch]);
            var rows = envelope.records.filter(function (row) {
                return row.enabled && typeof row.payload.value === "number";
            }).map(function (row) {
                return { key: row.payload.name, value: row.payload.value * 3 + row.id, active: true };
            });
            rows.forEach(function (row) { count++; total += row.value; });
            bytes += JSON.stringify(rows).length;
        }
    }
    return count + ":" + total + ":" + bytes;
}

function __velumBenchVerify() {
    return "1536:605580:71908";
}
