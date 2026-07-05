let value = 0;
try {
    value = 20;
} finally {
    value = value + 22;
}

if (value !== 42) {
    throw new Test262Error("finally did not run after normal try completion");
}

let caught = "none";
try {
    try {
        throw "try";
    } finally {
        throw "finally";
    }
} catch (error) {
    caught = error;
}

if (caught !== "finally") {
    throw new Test262Error("finally throw did not override try throw");
}

let pick = function() {
    try {
        return 1;
    } finally {
        return 42;
    }
};

if (pick() !== 42) {
    throw new Test262Error("finally return did not override try return");
}

42
