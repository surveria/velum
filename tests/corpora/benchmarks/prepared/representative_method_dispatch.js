// Stateful prototype methods with observable dispatch and two arguments.
var __velumBenchEvents;

function __velumBenchHandler(kind) {
    this.kind = kind;
    this.balance = kind * 7 + 1;
    this.count = 0;
}

__velumBenchHandler.prototype.apply = function (amount, sequence) {
    if (this.kind === 0) this.balance += amount * 2;
    else if (this.kind === 1) this.balance -= amount;
    else this.balance += sequence % 5;
    this.count++;
    return this.balance;
};

function __velumBenchSetup() {
    __velumBenchEvents = [];
    for (var i = 0; i < 8192; i++) {
        __velumBenchEvents.push({ kind: i % 3, amount: i % 31, sequence: i % 17 });
    }
}

function __velumBenchRun() {
    var handlers = [new __velumBenchHandler(0), new __velumBenchHandler(1), new __velumBenchHandler(2)];
    var observed = 0, balance = 0, count = 0;
    for (var i = 0; i < __velumBenchEvents.length; i++) {
        var event = __velumBenchEvents[i];
        observed += handlers[event.kind].apply(event.amount, event.sequence);
    }
    for (var kind = 0; kind < handlers.length; kind++) {
        balance += handlers[kind].balance * (kind + 1);
        count += handlers[kind].count;
    }
    return count + ":" + balance + ":" + observed;
}

function __velumBenchVerify() {
    return "8192:14996:62767175";
}
