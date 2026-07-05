let value = 0;
try {
    value = 20;
} finally {
    value = value + 22;
}

print(value);

let caught = "none";
try {
    try {
        throw "try";
    } finally {
        caught = "finally";
    }
} catch (error) {
    caught = caught + " " + error;
}

print(caught);

let pick = function() {
    try {
        return 1;
    } finally {
        return 42;
    }
};

print(pick());
