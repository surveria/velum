// Per-instance closures, five receiver shapes and a different event distribution.
var __velumBenchEvents;

function __velumBenchMakeHandler(kind, factor) {
    var calls = 0;
    var handler = { balance: kind * 11, kind: kind };
    handler["tag" + kind] = kind;
    handler.apply = function (event, adjustment) {
        calls++;
        this.balance += event.amount * factor + adjustment;
        return this.balance;
    };
    handler.calls = function () { return calls; };
    return handler;
}

function __velumBenchSetup() {
    __velumBenchEvents = [];
    for (var i = 0; i < 8192; i++) {
        __velumBenchEvents.push({ amount: i % 23, sequence: i % 13, kind: (i * 7) % 5 });
    }
}

function __velumBenchRun() {
    var factors = [2, -1, 3, -2, 1];
    var handlers = factors.map(function (factor, kind) {
        return __velumBenchMakeHandler(kind, factor);
    });
    var observed = 0, balance = 0, count = 0;
    __velumBenchEvents.forEach(function (event) {
        observed += handlers[event.kind].apply(event, event.sequence % 3);
    });
    handlers.forEach(function (handler, kind) {
        balance += handler.balance * (kind + 1);
        count += handler.calls();
    });
    return count + ":" + balance + ":" + observed;
}

function __velumBenchVerify() {
    return "8192:131367:50723067";
}
