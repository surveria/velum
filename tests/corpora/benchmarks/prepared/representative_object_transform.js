// Nested records, callbacks, object projection and stable multi-key sorting.
var __velumBenchRecords;

function __velumBenchSetup() {
    __velumBenchRecords = [];
    for (var i = 0; i < 128; i++) {
        __velumBenchRecords.push({
            id: i,
            active: i % 5 !== 0,
            payload: { amount: i % 17, tag: "event-" + i },
            weight: i % 7 + 1
        });
    }
}

function __velumBenchRun() {
    var count = 0, weighted = 0, text = 0;
    for (var batch = 0; batch < 32; batch++) {
        var projected = __velumBenchRecords.filter(function (row) {
            return row.active;
        }).map(function (row) {
            return {
                id: row.id,
                rank: row.id % 11,
                value: row.payload.amount * row.weight,
                label: row.payload.tag.replace("event", "row")
            };
        });
        projected.sort(function (left, right) {
            return left.rank - right.rank || left.id - right.id;
        });
        projected.forEach(function (row, index) {
            count++;
            weighted += (index + 1) * row.value;
            text += row.label.length;
        });
    }
    return count + ":" + weighted + ":" + text;
}

function __velumBenchVerify() {
    return "3264:5156768:20032";
}
