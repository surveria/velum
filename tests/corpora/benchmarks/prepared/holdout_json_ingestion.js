// Nested optional fields, escaping, Unicode and ordered explicit projection.
var __velumBenchDocuments;

function __velumBenchSetup() {
    __velumBenchDocuments = [];
    for (var batch = 0; batch < 32; batch++) {
        var records = [];
        for (var j = 0; j < 16; j++) {
            var id = batch * 16 + j;
            var row = { label: "camera \"" + id + "\"\nλ", id: id, meta: { payload: { value: id % 89 } } };
            if (id % 3 !== 0) row.weight = id % 5 + 1;
            records.push(row);
        }
        __velumBenchDocuments.push(JSON.stringify({ data: { rows: records }, tag: "holdout" }));
    }
}

function __velumBenchRun() {
    var count = 0, total = 0, bytes = 0;
    for (var repeat = 0; repeat < 4; repeat++) {
        for (var batch = __velumBenchDocuments.length - 1; batch >= 0; batch--) {
            var rows = JSON.parse(__velumBenchDocuments[batch]).data.rows;
            var projected = [];
            for (var j = 0; j < rows.length; j++) {
                var row = rows[j];
                if (row.id % 5 === 0) continue;
                var weight = row.weight === undefined ? 2 : row.weight;
                var value = row.meta.payload.value * weight;
                projected.push({ value: value, label: row.label, id: row.id });
                count++;
                total += value;
            }
            bytes += JSON.stringify(projected).length;
        }
    }
    return count + ":" + total + ":" + bytes;
}

function __velumBenchVerify() {
    return "1636:207612:82064";
}
