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
