// Wide trees, heterogeneous node shapes and iterative traversal.
var __velumBenchSeed;

function __velumBenchSetup() {
    __velumBenchSeed = 11;
}

function __velumBenchBuild() {
    var root = { id: 1, depth: 0, children: [], value: 12 };
    var queue = [root];
    for (var index = 0; index < queue.length; index++) {
        var parent = queue[index];
        if (parent.depth === 4) continue;
        for (var branch = 0; branch < 4; branch++) {
            var id = queue.length + 1;
            var child = { children: [], value: (id + __velumBenchSeed) % 89, depth: parent.depth + 1, id: id };
            child["tag" + branch] = branch;
            parent.children.push(child);
            queue.push(child);
        }
    }
    return root;
}

function __velumBenchRun() {
    var count = 0, weight = 0, text = 0;
    for (var repeat = 0; repeat < 12; repeat++) {
        var stack = [__velumBenchBuild()];
        while (stack.length !== 0) {
            var node = stack.pop();
            var transformed = { weight: node.value + node.depth * 5, label: "branch-" + node.id };
            count++;
            weight += transformed.weight * (node.depth + 1);
            text += transformed.label.length;
            for (var index = 0; index < node.children.length; index++) stack.push(node.children[index]);
        }
    }
    return count + ":" + weight + ":" + text;
}

function __velumBenchVerify() {
    return "4092:1226676:39624";
}
