var __velumBenchLast = 0;

function __velumBenchStep(value) {
    return ((value * 5) + 11) & 65535;
}

function __velumBenchSetup() {
    __velumBenchLast = 0;
    return __velumBenchLast;
}

function __velumBenchRun() {
    var value = 1;
    for (var index = 0; index < 262144; index = index + 1) {
        value = __velumBenchStep(value);
    }
    __velumBenchLast = value;
    return __velumBenchLast;
}

function __velumBenchVerify() {
    return 1;
}
