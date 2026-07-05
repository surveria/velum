let values = [40, 1, 2];
print(values.length);
print(values[0]);
print(values[2]);
print(values[9]);

let assigned = values[1] = values[0] + values[2];
print(assigned);
print(values[1]);

values[3] = assigned;
print(values.length);
print(values[3]);

values["01"] = 7;
print(values["01"]);
print(values.length);

let empty = [];
let trailing = [40, 2,];
print(empty.length);
print(trailing.length);
print(trailing[1]);
