import { Database } from "sqlite3";
import { apiConstants } from "../apiConstants";
import { bindSuggestionsDatabaseUtils } from "./bindSuggestionsDatabaseUtils";
import { databaseUtils } from "../../utils/databaseUtils";
import { Response } from "express";
import {
	BindSuggestionData,
	NewBindSuggestionData,
} from "../../utils/bindsModels";

export const bindSuggestionsApi = (app: any, database: Database) => {
	GET: app.get(
		`${apiConstants.API_BASE_PATH}${apiConstants.BINDS_SUGGESTIONS_PATH}/getBinds`,
		async (req: Request, res: Response) => {
			databaseUtils.wrapDatabaseTaskRequest(
				bindSuggestionsDatabaseUtils.getBindSuggestions(database),
				res
			);
		}
	);

	ADD: app.post(
		`${apiConstants.API_BASE_PATH}${apiConstants.BINDS_SUGGESTIONS_PATH}/addBind`,
		async (req: Request, res: Response) => {
			const bindToAdd = req.body as unknown as NewBindSuggestionData;
			databaseUtils.wrapDatabaseTaskRequest(
				bindSuggestionsDatabaseUtils.addBindSuggestion(database, bindToAdd),
				res
			);
		}
	);

	DELETE: app.post(
		`${apiConstants.API_BASE_PATH}${apiConstants.BINDS_SUGGESTIONS_PATH}/deleteBind`,
		async (req: Request, res: Response) => {
			const deleteBindData = req.body as unknown as BindSuggestionData;
			console.log(deleteBindData);
			databaseUtils.wrapDatabaseTaskRequest(
				bindSuggestionsDatabaseUtils.deleteBindSuggestion(
					database,
					deleteBindData
				),
				res
			);
		}
	);
};
