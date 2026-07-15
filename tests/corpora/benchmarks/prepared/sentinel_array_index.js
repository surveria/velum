var __velumBenchValues = [];
var __velumBenchLast = 0;

function __velumBenchSetup() {
    __velumBenchValues = [];
    for (var index = 0; index < 2048; index = index + 1) {
        __velumBenchValues.push((index * 17) & 255);
    }
    __velumBenchLast = 0;
    return __velumBenchLast;
}

function __velumBenchRun() {
    var total = 0;
    for (var round = 0; round < 128; round = round + 1) {
        for (var index = 0; index < __velumBenchValues.length; index = index + 1) {
            total = total + __velumBenchValues[index];
        }
    }
    __velumBenchLast = total;
    return __velumBenchLast;
}

function __velumBenchVerify() {
    return 33423360;
}
