var __rsqjsBenchText = "";
var __rsqjsBenchLast = 0;

function __rsqjsBenchSetup() {
    __rsqjsBenchText = "";
    for (var index = 0; index < 512; index = index + 1) {
        __rsqjsBenchText = __rsqjsBenchText + "safe-rust-js-0123456789";
    }
    __rsqjsBenchLast = 0;
    return __rsqjsBenchLast;
}

function __rsqjsBenchRun() {
    var total = 0;
    for (var round = 0; round < 4; round = round + 1) {
        for (var index = 0; index < __rsqjsBenchText.length; index = index + 1) {
            total = total + __rsqjsBenchText.charCodeAt(index);
        }
    }
    __rsqjsBenchLast = total;
    return __rsqjsBenchLast;
}

function __rsqjsBenchVerify() {
    return 3600384;
}
