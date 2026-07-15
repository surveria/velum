var __velumBenchLast = 0;

function __velumBenchSetup() {
    __velumBenchLast = 0;
    return __velumBenchLast;
}

function __velumBenchRun() {
    var total = 0;
    for (var index = 0; index < 262144; index = index + 1) {
        total = total + (((index * 3) + 7) & 255);
    }
    __velumBenchLast = total;
    return __velumBenchLast;
}

function __velumBenchVerify() {
    return 33423360;
}
