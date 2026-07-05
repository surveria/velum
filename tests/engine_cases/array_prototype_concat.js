let values = [1, 2];
let tail = [3, 4];
let object = { marker: 7 };
let result = values.concat(tail, 5, object);

let side = 0;
let marker = function() {
    side = 42;
    return [8, 9];
};
let sideResult = [7].concat(marker());

let sparse = Array(4);
sparse[1] = "one";
sparse[3] = "three";
let sparseResult = ["zero"].concat(sparse, "tail");

Array.prototype[0] = "proto-zero";
let inherited = Array(2);
inherited[1] = "own-one";
let inheritedResult = [].concat(inherited);
delete Array.prototype[0];

let plain = {};
plain[0] = "plain-zero";
plain.length = 1;
let plainResult = [1].concat(plain);

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("concat", result.length, result[0], result[1], result[2], result[3], result[4], result[5] === object);
print("source", values.length, values.join("|"), tail.join("|"));
print("side", side, sideResult.join("|"));
print("sparse", sparseResult.length, sparseResult[0], "1" in sparseResult, sparseResult[1], sparseResult[2], "3" in sparseResult, sparseResult[3], sparseResult[4], sparseResult[5], sparseResult.join("|"));
print("inherited", inheritedResult.length, inheritedResult[0], "0" in inheritedResult, inheritedResult[1]);
print("plain", plainResult.length, plainResult[0], plainResult[1] === plain);
print("meta", typeof Array.prototype.concat, Array.prototype.concat.name, Array.prototype.concat.length);
print("keys:" + prototypeKeys);
print("in", "concat" in values);

result.length === 6 &&
    result[0] === 1 &&
    result[1] === 2 &&
    result[2] === 3 &&
    result[3] === 4 &&
    result[4] === 5 &&
    result[5] === object &&
    values.join("|") === "1|2" &&
    tail.join("|") === "3|4" &&
    side === 42 &&
    sideResult.join("|") === "7|8|9" &&
    sparseResult.length === 6 &&
    sparseResult[0] === "zero" &&
    !("1" in sparseResult) &&
    sparseResult[1] === undefined &&
    sparseResult[2] === "one" &&
    !("3" in sparseResult) &&
    sparseResult[3] === undefined &&
    sparseResult[4] === "three" &&
    sparseResult[5] === "tail" &&
    sparseResult.join("|") === "zero||one||three|tail" &&
    inheritedResult.length === 2 &&
    inheritedResult[0] === "proto-zero" &&
    ("0" in inheritedResult) &&
    inheritedResult[1] === "own-one" &&
    plainResult.length === 2 &&
    plainResult[0] === 1 &&
    plainResult[1] === plain &&
    typeof Array.prototype.concat === "function" &&
    Array.prototype.concat.name === "concat" &&
    Array.prototype.concat.length === 1 &&
    prototypeKeys === "" &&
    ("concat" in values) ? 42 : 0
