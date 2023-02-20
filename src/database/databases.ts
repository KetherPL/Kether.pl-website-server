import { Database } from 'sqlite3';
import fs from 'fs';
import { bindsDatabaseUtils } from '../api/binds/bindsDatabaseUtils';
import { bindSuggestionsDatabaseUtils } from '../api/bindSuggestions/bindSuggestionsDatabaseUtils';
import { commandsDatabaseUtils } from '../api/commands/commandsDatabaseUtils';

const bindsDatabaseFilePath = './binds.sqlite';
const bindSuggestionsDatabaseFilePath = './bindSuggestions.sqlite';
const commandsDatabaseFilePath = './commands.sqlite';

const establishDBConnection = (
    databaseFileLoc: string,
    establisherFunction: (db: Database) => void
) => {
    if (fs.existsSync(databaseFileLoc)) {
        const db = new Database(databaseFileLoc);
        establisherFunction(db);
        return db;
    } else {
        const db = new Database(databaseFileLoc, (error) => {
            if (error) {
                return console.error(error.message);
            }
            establisherFunction(db);
        });
        console.log(
            `Connection with SQLite for ${databaseFileLoc} has been established`
        );
        return db;
    }
};

function verifyBindsTableExistance(db: Database) {
    db.exec(`
CREATE TABLE IF NOT EXISTS ${bindsDatabaseUtils.BINDS_DATABASE_REF} ( 
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 author VARCHAR (255) NOT NULL,
 text VARCHAR (255) NOT NULL,
 unique(text)
)
	`);

    db.exec(`
CREATE TABLE IF NOT EXISTS ${bindsDatabaseUtils.BIND_VOTINGS_DATABASE_REF} ( 
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 voterSteamID VARCHAR (255) NOT NULL,
 votedBindID VARCHAR (255) NOT NULL,
 vote VARCHAR (255) NOT NULL
)
	`);
}

function verifyBindSuggestionsTableExistance(db: Database) {
    db.exec(`
CREATE TABLE IF NOT EXISTS ${bindSuggestionsDatabaseUtils.BIND_SUGGESTIONS_DATABASE_REF} ( 
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 author VARCHAR (255) NOT NULL,
 text VARCHAR (255) NOT NULL,
 proposedBy VARCHAR (255) NOT NULL,
 unique(text)
)
	`);
}

function verifyCommandsTableExistance(db: Database) {
    db.exec(`
CREATE TABLE IF NOT EXISTS ${commandsDatabaseUtils.COMMANDS_DATABASE_REF} ( 
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 command VARCHAR (255) NOT NULL,
 description VARCHAR (512) NOT NULL,
 unique(command)
)
	`);
}

export const getBindsDatabase = (): Database => {
    return establishDBConnection(
        bindsDatabaseFilePath,
        verifyBindsTableExistance
    );
};

export const getCommandsDatabase = (): Database => {
    return establishDBConnection(
        commandsDatabaseFilePath,
        verifyCommandsTableExistance
    );
};

export const getBindSuggestionsDatabase = (): Database => {
    return establishDBConnection(
        bindSuggestionsDatabaseFilePath,
        verifyBindSuggestionsTableExistance
    );
};
