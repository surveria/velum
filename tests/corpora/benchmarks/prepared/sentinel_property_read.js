var __velumBenchRecord = {};
var __velumBenchLast = 0;

function __velumBenchSetup() {
    __velumBenchRecord = {
        alpha: 3,
        beta: 5,
        gamma: 7,
        delta: 11,
        epsilon: 13
    };
    __velumBenchLast = 0;
    return __velumBenchLast;
}

function __velumBenchRun() {
    var total = 0;
    for (var index = 0; index < 262144; index = index + 1) {
        total = total + __velumBenchRecord.alpha;
        total = total + __velumBenchRecord.beta;
        total = total + __velumBenchRecord.gamma;
        total = total + __velumBenchRecord.delta;
        total = total + __velumBenchRecord.epsilon;
    }
    __velumBenchLast = total;
    return __velumBenchLast;
}

function __velumBenchVerify() {
    return 10223616;
}
