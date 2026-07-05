let values = [1, 2, 3, 4];
let returned = values.reverse();
let sameObject = returned === values;

let odd = [1, 2, 3];
let oddReturned = odd.reverse();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let sideCopy = [7];
let sideReturn = sideCopy.reverse(marker());

let sparse = Array(4);
sparse[1] = "one";
sparse[3] = "three";
let sparseReturn = sparse.reverse();

Array.prototype[2] = "proto-two";
let inheritedUpper = Array(3);
let inheritedUpperReturn = inheritedUpper.reverse();
delete Array.prototype[2];

Array.prototype[0] = "proto-zero";
let inheritedLower = Array(3);
let inheritedLowerReturn = inheritedLower.reverse();
delete Array.prototype[0];

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("reverse", sameObject, values.join("|"), values.length, oddReturned === odd, odd.join("|"));
print("side", side, sideReturn === sideCopy, sideCopy.join("|"));
print("sparse", sparse.length, sparse[0], "1" in sparse, sparse[2], "3" in sparse, sparse.join("|"), sparseReturn === sparse);
print("inherited-upper", inheritedUpperReturn === inheritedUpper, inheritedUpper[0], "0" in inheritedUpper, inheritedUpper[2], "2" in inheritedUpper);
print("inherited-lower", inheritedLowerReturn === inheritedLower, inheritedLower[0], "0" in inheritedLower, inheritedLower[2], "2" in inheritedLower);
print("meta", typeof Array.prototype.reverse, Array.prototype.reverse.name, Array.prototype.reverse.length);
print("keys:" + prototypeKeys);
print("in", "reverse" in values);

sameObject &&
    values.join("|") === "4|3|2|1" &&
    values.length === 4 &&
    oddReturned === odd &&
    odd.join("|") === "3|2|1" &&
    side === 42 &&
    sideReturn === sideCopy &&
    sideCopy.join("|") === "7" &&
    sparse.length === 4 &&
    sparse[0] === "three" &&
    !("1" in sparse) &&
    sparse[2] === "one" &&
    !("3" in sparse) &&
    sparse.join("|") === "three||one|" &&
    sparseReturn === sparse &&
    inheritedUpperReturn === inheritedUpper &&
    inheritedUpper[0] === "proto-two" &&
    ("0" in inheritedUpper) &&
    inheritedUpper[2] === undefined &&
    !("2" in inheritedUpper) &&
    inheritedLowerReturn === inheritedLower &&
    inheritedLower[0] === undefined &&
    !("0" in inheritedLower) &&
    inheritedLower[2] === "proto-zero" &&
    ("2" in inheritedLower) &&
    typeof Array.prototype.reverse === "function" &&
    Array.prototype.reverse.name === "reverse" &&
    Array.prototype.reverse.length === 0 &&
    prototypeKeys === "" &&
    ("reverse" in values) ? 42 : 0
