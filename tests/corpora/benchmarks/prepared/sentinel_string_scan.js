var __velumBenchText = "";
var __velumBenchLast = 0;

function __velumBenchSetup() {
    __velumBenchText = "";
    for (var index = 0; index < 512; index = index + 1) {
        __velumBenchText = __velumBenchText + "safe-rust-js-0123456789";
    }
    __velumBenchLast = 0;
    return __velumBenchLast;
}

function __velumBenchRun() {
    var total = 0;
    for (var round = 0; round < 16; round = round + 1) {
        for (var index = 0; index < __velumBenchText.length; index = index + 1) {
            total = total + __velumBenchText.charCodeAt(index);
        }
    }
    __velumBenchLast = total;
    return __velumBenchLast;
}

function __velumBenchVerify() {
    return 14401536;
}
