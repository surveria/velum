// Mixed record shapes, optional fields and explicit loops instead of callbacks.
var __velumBenchRecords;

function __velumBenchSetup() {
    __velumBenchRecords = [];
    for (var i = 0; i < 160; i++) {
        var payload = { tag: "sample " + i, amount: i % 19 };
        var row = i % 2 === 0
            ? { payload: payload, id: i, active: i % 7 !== 0 }
            : { active: i % 7 !== 0, id: i, payload: payload };
        if (i % 4 !== 0) row.weight = i % 5 + 1;
        __velumBenchRecords.push(row);
    }
}

function __velumBenchRun() {
    var count = 0, weighted = 0, text = 0;
    for (var batch = 0; batch < 32; batch++) {
        var projected = [];
        var index = 0;
        while (index < __velumBenchRecords.length) {
            var row = __velumBenchRecords[index++];
            if (!row.active) continue;
            projected.push({
                rank: row.id % 13,
                value: row.payload.amount * (row.weight === undefined ? 1 : row.weight),
                label: row.payload.tag.split(" ").join("_"),
                id: row.id
            });
        }
        projected.sort(function (left, right) {
            return right.rank - left.rank || right.id - left.id;
        });
        for (var j = 0; j < projected.length; j++) {
            count++;
            weighted += (j + 1) * projected[j].value;
            text += projected[j].label.length;
        }
    }
    return count + ":" + weighted + ":" + text;
}

function __velumBenchVerify() {
    return "4384:6526912:40864";
}
