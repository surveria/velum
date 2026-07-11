let assert = {};

assert.throws = function (expectedErrorConstructor, callback, message) {
    let threw = false;
    let error = undefined;
    try {
        callback();
    } catch (caught) {
        threw = true;
        error = caught;
    }
    if (threw !== true) {
        throw new Error(message || "assert.throws expected an exception, but no exception was thrown");
    }
    if (error instanceof expectedErrorConstructor || expectedErrorConstructor.name === error.name) {
        return;
    }
    throw new Error(
        message ||
            "assert.throws expected " + expectedErrorConstructor.name + ", got " + error.name
    );
};
