var __rsqjsBenchLast = 0;

function __rsqjsBenchSetup() {
    __rsqjsBenchLast = 0;
    return __rsqjsBenchLast;
}

function __rsqjsBenchRun() {
    var total = 0;
    for (var index = 0; index < 262144; index = index + 1) {
        total = total + (((index * 3) + 7) & 255);
    }
    __rsqjsBenchLast = total;
    return __rsqjsBenchLast;
}

function __rsqjsBenchVerify() {
    return 33423360;
}
