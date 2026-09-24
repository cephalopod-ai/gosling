export type Permission =
  | 'always_allow'
  | 'always_allow_domain'
  | 'allow_folder_for_session'
  | 'always_allow_folder'
  | 'allow_once'
  | 'cancel'
  | 'deny_once'
  | 'always_deny';
