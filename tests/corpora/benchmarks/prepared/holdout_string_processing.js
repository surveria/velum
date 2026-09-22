// Unicode, empty fields and variable-width lines with a different delimiter.
var __velumBenchLines;

function __velumBenchSetup() {
    __velumBenchLines = [];
    for (var i = 0; i < 512; i++) {
        var label = i % 4 === 0 ? "" : "камера-" + i + "😀";
        __velumBenchLines.push(" zone" + (i % 11) + ";" + label + ";" + (i % 83) + "; λ\t active " + (i % 5) + " ");
    }
}

function __velumBenchRun() {
    var length = 0, weighted = 0, grouped = 0;
    for (var repeat = 0; repeat < 8; repeat++) {
        var groups = new Map();
        __velumBenchLines.forEach(function (line, index) {
            var fields = line.trim().split(";");
            var category = fields.shift().toLowerCase();
            var value = Number(fields[1]);
            var normalized = [fields[0] || "missing", fields[2].trim().replace(/\s+/g, "_")].join(":");
            groups.set(category, (groups.get(category) || 0) + value);
            length += normalized.length;
            weighted += (index + 1) * value;
        });
        var keys = Array.from(groups.keys()).sort();
        for (var i = 0; i < keys.length; i++) grouped += (i + 1) * groups.get(keys[i]);
    }
    return length + ":" + weighted + ":" + grouped;
}

function __velumBenchVerify() {
    return "88432:43410968:977048";
}
