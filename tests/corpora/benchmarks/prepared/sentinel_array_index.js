var __rsqjsBenchValues = [];
var __rsqjsBenchLast = 0;

function __rsqjsBenchSetup() {
    __rsqjsBenchValues = [];
    for (var index = 0; index < 2048; index = index + 1) {
        __rsqjsBenchValues.push((index * 17) & 255);
    }
    __rsqjsBenchLast = 0;
    return __rsqjsBenchLast;
}

function __rsqjsBenchRun() {
    var total = 0;
    for (var round = 0; round < 128; round = round + 1) {
        for (var index = 0; index < __rsqjsBenchValues.length; index = index + 1) {
            total = total + __rsqjsBenchValues[index];
        }
    }
    __rsqjsBenchLast = total;
    return __rsqjsBenchLast;
}

function __rsqjsBenchVerify() {
    return 33423360;
}
