export enum SelectorType {
    ALL = '*',
    COUNT_ALL = 'COUNT(*)',
}

export class SqlQueryBuilder {
    query: string = ``;

    constructor() {}

    addSpace() {
        this.query += ` `;
        return this;
    }

    build() {
        this.query += `;`;
        return this.query;
    }

    select(selector: SelectorType | string) {
        this.query += `SELECT ${selector}`;
        this.addSpace();
        return this;
    }

    selectMultiple(selector: string[]) {
        this.query += `SELECT ${selector.join(`,`)}`;
        this.addSpace();
        return this;
    }

    or() {
        this.query += `OR `;
        this.addSpace();
        return this;
    }

    selectDistinct(selector: SelectorType | string) {
        this.query += `SELECT DISTINCT ${selector}`;
        this.addSpace();
        return this;
    }

    update(inTableName: string) {
        this.query += `UPDATE ${inTableName}`;
        this.addSpace();
        return this;
    }

    set(updateData: { columnName: string; columnValue: string | number }[]) {
        this.query += `SET`;
        this.addSpace();
        updateData.forEach((data, index) => {
            this.query += `${data.columnName}='${data.columnValue}'`;
            if (index !== updateData.length - 1) {
                this.query += `,`;
            }
        });
        this.addSpace();
        return this;
    }

    deleteFrom(fromTableName: string) {
        this.query += `DELETE FROM ${fromTableName}`;
        this.addSpace();
        return this;
    }

    from(tableName: string) {
        this.query += `FROM ${tableName}`;
        return this.addSpace();
        return this;
    }

    as(as: string) {
        this.query += `AS ${as}`;
        return this.addSpace();
        return this;
    }

    whereColumnName(columnName: string) {
        this.query += `WHERE ${columnName}`;
        return this.addSpace();
        return this;
    }

    columnNameWithoutWhere(columnName: string) {
        this.query += `${columnName}`;
        return this.addSpace();
        return this;
    }

    equals(value: string) {
        this.query += `= ${value}`;
        return this.addSpace();
        return this;
    }

    matches(matchString: string) {
        this.query += `REGEXP '${matchString}'`;
        return this.addSpace();
        return this;
    }

    offset(offset: number | string) {
        this.query += `OFFSET ${offset}`;
        return this.addSpace();
        return this;
    }

    limit(limit: number | string) {
        this.query += `LIMIT ${limit}`;
        return this.addSpace();
        return this;
    }

    orderBy(orderByValue: string, orderDirection: 'asc' | 'desc') {
        this.query += `ORDER BY ${orderByValue} ${orderDirection}`;
        return this.addSpace();
        return this;
    }

    insertIntoTableValues(
        tableNameIn: string,
        params: { columnName: string; value: string }[]
    ) {
        this.query += `INSERT INTO ${tableNameIn}`;
        this.addSpace();
        const columnNames: string[] = [];
        const values: string[] = [];
        params.forEach((param) => {
            columnNames.push(param.columnName);
            values.push(param.value);
        });
        this.query +=
            '(' +
            columnNames.join(', ') +
            ')' +
            ' ' +
            'VALUES' +
            ' ' +
            '(' +
            values.map((value) => `'${value}'`).join(', ') +
            ')';
        this.addSpace();
        return this;
    }

    whereMultipleColumnsEquals(
        params: { columnName: string; value: string }[]
    ) {
        const columnNames: string[] = [];
        const values: string[] = [];
        params.forEach((param) => {
            columnNames.push(param.columnName);
            values.push(param.value);
        });
        this.query += `WHERE `;
        columnNames.forEach((columnName: string, index: number) => {
            if (index === 0) {
                this.query += `${columnName} = ${values[index]} `;
            } else {
                this.query += `AND ${columnName} = ${values[index]} `;
            }
        });
        this.addSpace();
        return this;
    }
}
