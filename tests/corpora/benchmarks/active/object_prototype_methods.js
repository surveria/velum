var op = Object.prototype;
var samples = [[], {}, 42, "s", true, new Date(), /x/, new Error("e"), function () {}];
var total = 0;

for (var round = 0; round < 4000; round++) {
    for (var i = 0; i < samples.length; i++) {
        total += op.toString.call(samples[i]).length;
    }

    var proto = Object.create(op);
    var child = Object.create(proto);
    if (proto.isPrototypeOf(child)) {
        total += 1;
    }
    if (!child.isPrototypeOf(proto)) {
        total += 1;
    }

    var entries = Object.fromEntries([["a", round], ["b", round + 1], ["c", round + 2]]);
    total += entries.a + entries.b + entries.c;
    total += op.valueOf.call(round) instanceof Number ? 1 : 0;
}

total
