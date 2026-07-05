let values = [1, 2, 3, 4];
let returned = values.reverse();

let odd = [1, 2, 3];
let oddReturned = odd.reverse();

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

returned === values &&
    values.join("|") === "4|3|2|1" &&
    oddReturned === odd &&
    odd.join("|") === "3|2|1" &&
    sparseReturn === sparse &&
    sparse[0] === "three" &&
    !("1" in sparse) &&
    sparse[2] === "one" &&
    !("3" in sparse) &&
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
    Array.prototype.reverse.length === 0
    ? 42
    : 0
