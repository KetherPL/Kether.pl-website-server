import mysql, { Connection, queryCallback } from 'mysql';

export var liveServerDatabaseUtils = {
    performQuery: (query: string, callback?: queryCallback) => {
        performLiveQuery(query, callback);
    },
};

var mysqlConnection: Connection;

const getOrCreateConnection = () => {
    if (!mysqlConnection) {
        mysqlConnection = mysql.createConnection({
            host: '',
            port: 3306,
            user: '',
            password: '',
            database: '',
            connectTimeout: 10 * 1000,
            timeout: 3 * 1000,
        });
    }
    return mysqlConnection;
};

const dbConnection = () => getOrCreateConnection();

const getConnection = (tryCount: number = 3) => {
    let connection = dbConnection();
    let tries = 1;
    while (tries <= tryCount && tries > 0 && tryCount > tries) {
        if (!connection) {
            connection = dbConnection();
            tries++;
        } else {
            return connection;
        }
    }
    throw new Error(`Could not establish db connection after #${tries} tries`);
};
const performLiveQuery = (query: string, callback?: queryCallback) => {
    const connection = getConnection();
    if (!connection) {
        return;
    }

    connection.query(query, (err, results, fields) => {
        if (err) {
            callback && callback(err, undefined, undefined);
        }
        callback && callback(null, results, fields);
        connection.resume();
    });
};
