// Delimited application logs: trimming, slicing, replacement and grouping.
var __velumBenchLines;

function __velumBenchSetup() {
    __velumBenchLines = [];
    for (var i = 0; i < 512; i++) {
        __velumBenchLines.push("  group" + (i % 7) + "|sensor-" + i + "|" + (i % 101) + "|status  ready  " + (i % 3));
    }
}

function __velumBenchRun() {
    var length = 0, weighted = 0, grouped = 0;
    for (var repeat = 0; repeat < 8; repeat++) {
        var groups = Object.create(null);
        for (var i = 0; i < __velumBenchLines.length; i++) {
            var line = __velumBenchLines[i].trim();
            var separator = line.indexOf("|");
            var category = line.slice(0, separator).toUpperCase();
            var fields = line.slice(separator + 1).split("|");
            var normalized = fields[0] + ":" + fields[2].replace(/ +/g, "_");
            var value = Number(fields[1]);
            groups[category] = (groups[category] || 0) + value;
            length += normalized.length;
            weighted += (i + 1) * value;
        }
        Object.keys(groups).sort().forEach(function (key, index) {
            grouped += (index + 1) * groups[key];
        });
    }
    return length + ":" + weighted + ":" + grouped;
}

function __velumBenchVerify() {
    return "101520:54625736:807104";
}
