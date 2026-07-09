var __rsqjsBenchLast = 0;

function __rsqjsBenchStep(value) {
    return ((value * 5) + 11) & 65535;
}

function __rsqjsBenchSetup() {
    __rsqjsBenchLast = 0;
    return __rsqjsBenchLast;
}

function __rsqjsBenchRun() {
    var value = 1;
    for (var index = 0; index < 262144; index = index + 1) {
        value = __rsqjsBenchStep(value);
    }
    __rsqjsBenchLast = value;
    return __rsqjsBenchLast;
}

function __rsqjsBenchVerify() {
    return 1;
}
