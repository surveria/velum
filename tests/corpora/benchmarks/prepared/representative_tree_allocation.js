// Allocate, recursively transform and visit a fresh bounded binary tree.
var __velumBenchSeed;

function __velumBenchSetup() {
    __velumBenchSeed = 7;
}

function __velumBenchBuild(depth, id) {
    return {
        value: (id + __velumBenchSeed) % 97,
        label: "node-" + id,
        children: depth === 8 ? [] : [__velumBenchBuild(depth + 1, id * 2), __velumBenchBuild(depth + 1, id * 2 + 1)]
    };
}

function __velumBenchTransform(node, depth, adjust) {
    return {
        weight: adjust(node.value, depth),
        label: node.label.toUpperCase(),
        children: node.children.map(function (child) {
            return __velumBenchTransform(child, depth + 1, adjust);
        })
    };
}

function __velumBenchVisit(node, depth, totals) {
    totals.count++;
    totals.weight += node.weight * (depth + 1);
    totals.text += node.label.length;
    node.children.forEach(function (child) { __velumBenchVisit(child, depth + 1, totals); });
}

function __velumBenchRun() {
    var totals = { count: 0, weight: 0, text: 0 };
    for (var repeat = 0; repeat < 8; repeat++) {
        var tree = __velumBenchBuild(0, 1);
        var transformed = __velumBenchTransform(tree, 0, function (value, depth) { return value + depth * 3; });
        __velumBenchVisit(transformed, 0, totals);
    }
    return totals.count + ":" + totals.weight + ":" + totals.text;
}

function __velumBenchVerify() {
    return "4088:2263400:31840";
}
