var __rsqjsBenchRecord = {};
var __rsqjsBenchLast = 0;

function __rsqjsBenchSetup() {
    __rsqjsBenchRecord = {
        alpha: 3,
        beta: 5,
        gamma: 7,
        delta: 11,
        epsilon: 13
    };
    __rsqjsBenchLast = 0;
    return __rsqjsBenchLast;
}

function __rsqjsBenchRun() {
    var total = 0;
    for (var index = 0; index < 262144; index = index + 1) {
        total = total + __rsqjsBenchRecord.alpha;
        total = total + __rsqjsBenchRecord.beta;
        total = total + __rsqjsBenchRecord.gamma;
        total = total + __rsqjsBenchRecord.delta;
        total = total + __rsqjsBenchRecord.epsilon;
    }
    __rsqjsBenchLast = total;
    return __rsqjsBenchLast;
}

function __rsqjsBenchVerify() {
    return 10223616;
}
