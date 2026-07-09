let total = 0;

for (let round = 0; round < 4096; round = round + 1) {
    for (let index = 0; index < 1024; index = index + 1) {
        try {
            if ((index & 3) === 0) {
                throw 1;
            }
            total = total + 1;
        } catch (error) {
            total = total + error;
        } finally {
            total = total + 1;
        }
    }
}

total;
