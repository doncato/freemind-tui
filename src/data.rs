pub(crate) mod data_types {
    use std::{fmt, io::Cursor, str, collections::{HashMap, hash_map::Iter}};
    use crossterm::event::KeyCode;
    use serde::{Serialize, Deserialize};
    use reqwest::{Client, Response, header::HeaderValue};
    use quick_xml::{Reader, events::{attributes::Attribute, Event, BytesStart, BytesText, BytesEnd}, Writer};
    use rand::Rng;
    use tui::{widgets::{ListItem, ListState, TableState}, style::{Modifier, Style}};

    /// Gets the value of the id attribute of any node
    fn get_id_attribute<'a>(reader: &Reader<&[u8]>, element: &BytesStart<'a>) -> Option<u16> {
        element
            .attributes()
            .into_iter()
            .filter_map(|f| f.ok())
            .filter(|e| e.key.local_name().as_ref() == b"id")
            .map(|v| {
                if let Ok(mut attr) = v.decode_and_unescape_value(&reader) {
                    attr = attr.into();
                    attr.parse::<u16>().ok()
                } else {
                    None
                }
            })
            .next()
            .flatten()
    }

    #[derive(Debug, Clone)]
    enum RegistryNode {
        Element(EntryNode),
        Directory(DirectoryNode),
    }

    /*
    impl<'t> FromIterator<_, T> for Node<'t> {
        fn from_iter<T>(_: T) -> Self where T: IntoIterator, std::iter::IntoIterator::Item = T {
            todo!() 
        }
    }
    */

    impl RegistryNode {
        /// Returns the Name of the node if available
        fn name(&self) -> Option<String> {
            match self {
                Self::Element(e) => Some(e.to_string()),
                Self::Directory(d) => None
            }
        }

        /// Returns the ID of the node
        fn id(&self) -> Option<u16> {
            match self {
                Self::Element(e) => e.id,
                Self::Directory(d) => d.id
            }
        }

        /// Returns whether the node was removed
        fn removed(&self) -> bool {
            match self {
                Self::Element(e) => e.removed,
                Self::Directory(_) => false,
            }
        }

        /// Returns the AppElement if it is an Element or None
        fn element(&self) -> Option<&EntryNode> {
            match self {
                Self::Element(e) => Some(e),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct Registry {
        nodes: Vec<RegistryNode>,
    }

    impl Registry {
        fn empty() -> Self {
            Self { nodes: Vec::new() }
        }

        /// Reads with the given xml reader the content into an NodeName and NodeValue
        fn read_node<'t>(mut reader: Reader<&'t [u8]>, parent: &[u8]) -> Result<(Reader<&'t [u8]>, NodeValue), quick_xml::Error> {
            let mut buf: Vec<u8> = Vec::new();

            let mut result: NodeValue = NodeValue::Text("".to_string());

            let mut read_values: HashMap<NodeName, NodeValue> = HashMap::new();

            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) => {
                        let found: NodeValue;
                        (reader, found) = Self::read_node(reader, e.name().as_ref())?;
                        read_values.insert(
                            NodeName::from_str(str::from_utf8(e.name().as_ref()).unwrap_or("")),
                            found
                        );
                    }
                    Event::Text(e) => {
                        result = NodeValue::Text(
                            str::from_utf8(
                                e.into_inner().as_ref()
                            )
                            .unwrap_or("")
                            .to_string()
                        )
                    },
                    Event::End(e) if e.name().as_ref() == parent => {
                        break
                    },
                    Event::Eof => break,
                    _ => ()

                }
            };
            if read_values.is_empty() {
                Ok((reader, result))
            } else {
                Ok((reader, NodeValue::NestedNode(read_values)))
            }
        }

        /// Parses the xml document as a String into the Registry object
        fn from_string(mut self, xml: &String) -> Result<Self, quick_xml::Error> {
            let mut reader: Reader<&[u8]> = Reader::from_str(xml);
            reader.trim_text(true);

            let mut in_element: Option<u16> = None;
            let mut in_directory: Option<u16> = None;
            //let mut inside: String = "".to_string();
            let mut next_map: HashMap<NodeName, NodeValue> = HashMap::new();
            let mut next_nodes: Vec<RegistryNode> = Vec::new();
            let mut buf: Vec<u8> = Vec::new();

            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.name().as_ref() == b"entry" => {
                        in_element = get_id_attribute(&reader, &e);
                    }
                    Event::End(e) if e.name().as_ref() == b"entry" => {
                        if in_directory.is_some() {
                            next_nodes.push(RegistryNode::Element(EntryNode::new(in_element, next_map.clone())));
                            next_map.clear();
                            in_element = None;
                        } else if in_element.is_some() {
                            self.nodes.push(RegistryNode::Element(EntryNode::new(in_element, next_map.clone())));
                            next_map.clear();
                            in_element = None;
                        }

                    }
                    Event::Start(e) if e.name().as_ref() == b"directory" => {
                        in_directory = get_id_attribute(&reader, &e);
                    }
                    Event::End(e) if e.name().as_ref() == b"directory" => {
                        if in_directory.is_some() {
                            self.nodes.push(
                                RegistryNode::Directory(
                                    DirectoryNode::new(in_directory, next_map.clone(), next_nodes.clone())
                                )
                            );
                            next_map.clear();
                            next_nodes.clear();
                            in_directory = None;
                        }
                    }
                    Event::Start(e) if in_element.is_some() => {
                        let found: NodeValue;
                        (reader, found) = Self::read_node(reader, e.name().as_ref())?;
                        next_map.insert(
                            NodeName::from_str(str::from_utf8(e.name().as_ref()).unwrap_or("")),
                            found
                        );
                    }

                    Event::End(e) if e.name().as_ref() == b"registry" => break,
                    Event::Eof => break,
                    _ => (),
                }
            }

            Ok(self)
        }

        fn entries(&self) -> Vec<&EntryNode> {
            self.nodes
                .iter()
                .map(|e| e.element())
                .filter(|e| e.is_some())
                .map(|e| e.unwrap())
                .collect()
        }
    }


    #[derive(Debug, Clone)]
    struct DirectoryNode {
        id: Option<u16>,
        nodes: HashMap<NodeName, NodeValue>,
        content: Vec<RegistryNode>,
    }

    impl DirectoryNode {
        fn new(id: Option<u16>, nodes: HashMap<NodeName, NodeValue>, content: Vec<RegistryNode>) -> Self {
            Self {
                id,
                nodes,
                content,
            }
        }
    }

    /// The Keynames for different Nodes
    #[derive(PartialEq, Eq, Hash, Debug, Clone)]
    pub enum NodeName {
        /// The title
        Title,
        /// A description
        Description,
        /// The Location aka. where it takes place
        Location,
        /// A color which can optionally be used for display purposes
        Color,
        /// A due date
        Due,
        /// How long it takes place in minutes
        Duration,
        /// How to trigger an alert
        Alert,
        /// Any other elements
        Other(String),
    }

    impl fmt::Display for NodeName {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", match self {
                Self::Title => "name",
                Self::Description => "description",
                Self::Location => "location",
                Self::Color => "color",
                Self::Due => "due",
                Self::Duration => "duration",
                Self::Alert => "alert",
                Self::Other(val) => val,
                _ => "",
            })
        }
    }

    impl NodeName {
        pub fn from_str(s: &str) -> Self {
            match s.to_lowercase().as_str() {
                "name" => Self::Title,
                "description" => Self::Description,
                "location" => Self::Location,
                "color" => Self::Color,
                "due" => Self::Due,
                "duration" => Self::Duration,
                "alert" => Self::Alert,
                e => Self::Other(e.to_string())
            }
        }

        pub fn order(&self) -> u8 {
            match self {
                Self::Title => 0,
                Self::Description => 1,
                Self::Location => 2,
                Self::Due => 3,
                Self::Duration => 4,
                Self::Other(v) => 127 + (v.len() % 128).try_into().unwrap_or(0),
                Self::Color => 254,
                Self::Alert => 254,
                _ => u8::MAX,
            }
        }
    }

    trait Node {
        // Writes the element using the given quick xml writer
        /// skips silently if the element does not have an ID
        /// Skips the outer 'entry' tags if 'with_head' is false
        fn write<W: std::io::Write>(&self, writer: &mut Writer<W>, with_head: bool) -> Result<(), quick_xml::Error>;
    }

    /// The Value of a subnode inside an entry node
    /// aka. the text in between a subnode of an entry node
    #[derive(Debug, Clone, PartialEq)]
    pub enum NodeValue {
        Text(String),
        NestedNode(HashMap<NodeName, NodeValue>),
    }

    /*
    impl ToString for NodeValue {
        fn to_string(&self) -> String {
            match self {
                Self::Text(t) => {
                    t.to_string()
                },
                Self::NestedNode(n) => {
                    n.iter()
                        .map(|e| e.name().to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                }
            }
        }
    }
    */

    impl fmt::Display for NodeValue {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                NodeValue::Text(t) => {
                    write!(f, "{}", t)
                },
                NodeValue::NestedNode(n) => {
                    n
                        .iter()
                        .for_each(|(k, v)| {
                            write!(f, "{} - {}\n", k, v).unwrap(); // FIXME CHANGEME RETARD
                        });
                    Ok(())
                }
            }
        }
    }

    impl NodeValue {
        /// Gets the contained text or composes the full XML text if other
        /// nodes are contained
        fn text_raw(&self) -> String {
            match self {
                Self::Text(t) => {
                    t.to_string()
                },
                Self::NestedNode(n) => {
                    n
                        .iter()
                        .map(|(n, v)| {
                            format!("<{}>{}</{}>", n, v.text_raw(), n)
                        })
                        .collect::<Vec<String>>()
                        .join("")
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct EntryNode {
        id: Option<u16>,
        nodes: HashMap<NodeName, NodeValue>,
        removed: bool,
        modified: bool,
    }

    impl PartialEq for EntryNode {
        fn eq(&self, other: &Self) -> bool {
            match self.id {
                Some(id) => Some(id) == other.id,
                None => self == other, // Isn't this recursive???
            }
        }
    }

    impl AsRef<Self> for EntryNode {
        fn as_ref(&self) -> &Self {
            return &self;
        }
    }

    impl fmt::Display for EntryNode {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            /*
            let disp_due: String = match self.nodes.get("due") {
                Some(due) => {
                    let due_timestamp: i64 = due.parse().unwrap_or(-1);
                    let utc_due: String = match Utc.timestamp_opt(due_timestamp, 0) {
                        LocalResult::None => "None".to_string(),
                        LocalResult::Single(val) => val.with_timezone(&chrono::Local).to_rfc2822(),
                        LocalResult::Ambiguous(val, _) => val.with_timezone(&chrono::Local).to_rfc2822(),
                    };
                    utc_due
                },
                None => "None".to_string()
            };
            */
        
            let id: String = match self.id {
                Some(id) => format!("{}", id),
                None => "None".to_string()
            };
            write!(
                f,
                "ID: {}",
                id,
            )
        }
    }

    impl Node for EntryNode {
        /// Writes the element using the given quick xml writer
        /// skips silently if the element does not have an ID
        /// Skips the outer 'entry' tags if 'with_head' is false
        fn write<W: std::io::Write>(&self, writer: &mut Writer<W>, with_head: bool) -> Result<(), quick_xml::Error> {
            if self.id.is_none() {
                return Ok(());
            }
            if with_head {
                writer.write_event(Event::Start(
                    BytesStart::new("entry")
                        .with_attributes([Attribute::from(("id", self.id.unwrap().to_string().as_str()))])
                    )
                )?;
            }

            for (key, value) in self.nodes.iter() {
                writer.write_event(Event::Start(BytesStart::new(key.to_string())))?;
                writer.write_event(Event::Text(BytesText::new(&value.text_raw())))?;
                writer.write_event(Event::End(BytesEnd::new(key.to_string())))?;
            }

            if with_head {
                writer.write_event(Event::End(BytesEnd::new("entry")))?;
            }

            Ok(())
        }
    }

    impl EntryNode {
        pub fn new(id: Option<u16>, nodes: HashMap<NodeName, NodeValue>) -> Self {
            Self {
                id,
                nodes,
                removed: false,
                modified: false,
            }
        }

        pub fn title(&self) -> Option<String> {
            Some(self.nodes.get(&NodeName::Title)?.to_string())
        }

        pub fn description(&self) -> Option<String> {
            Some(self.nodes.get(&NodeName::Description)?.to_string())
        }

        pub fn due(&self) -> Option<u32> {
            self.nodes.get(&NodeName::Due)?
                .to_string()
                .parse::<u32>()
                .ok()
        }
        
        /// Returns the number of nodes held by the EntryNode
        pub fn node_count(&self) -> usize {
            self.nodes.len()
        }

        /// Returns the number of nodes and subnodes held by the EntryNode
        pub fn flattened_node_count(&self) -> usize {
            let mut node_vec = self.nodes
                .iter()
                .collect();
            return Self::flatten_tree(&mut Vec::new(), &node_vec).len();
        }

        /// Returns all nodes of the element
        pub fn nodes(&mut self) -> &mut HashMap<NodeName, NodeValue> {
            &mut self.nodes
        }

        /// Returns the nodes of the element as an Iter
        pub fn get_nodes(&self) -> Iter<NodeName, NodeValue> {
            return self.nodes.iter()
        }

        /// Sets this element to modified
        pub fn modified(&mut self) {
            self.modified = true;
        }

        /// Returns the nested tree as an Vec over each nested level
        fn flatten_tree<'a>(name_stack: &mut Vec<&'a NodeName>, nodes: &Vec<(&'a NodeName, &'a NodeValue)>) -> Vec<(String, String)> {
            let mut r: Vec<(String, String)> = Vec::new();
            nodes
                .iter()
                .for_each(|(node_name, node_value)| {
                    name_stack.push(node_name);
                    match node_value {
                        NodeValue::Text(t) => {
                            r.push(
                                (
                                    name_stack
                                        .iter()
                                        .map(|e| e.to_string())
                                        .collect::<Vec<String>>()
                                        .join("→"),
                                    t.to_string()
                                )
                            );
                        },
                        NodeValue::NestedNode(n) => {
                            r.append(&mut Self::flatten_tree(name_stack, &n.iter().collect()))
                        }
                    }
                    name_stack.pop();
                });

            r
        }

        /// A function that returns the App Element in a Form of Vectors
        /// where each element is another vector consisting of the key
        /// and the value. Also sorts the elements
        pub fn get_vecs(&self) -> Vec<(String, String)> {
            let mut nodes_sorted: Vec<(&NodeName, &NodeValue)> = self.nodes
                .iter()
                .collect();
                
            nodes_sorted.sort_by(|a, b| a.0.order().cmp(&b.0.order()));

            return Self::flatten_tree(&mut Vec::new(), &nodes_sorted);
        }

        /// A function that returns the title followed by the description
        /// followed by the tags as a single lowercase string, this is designed
        /// for usage of searching and filtering
        pub fn get_text(&self) -> String {
            return String::new() 
                + &self.nodes.clone().into_values().map(|e| e.to_string()).collect::<Vec<String>>().join(" ")
                .to_lowercase();
        }

        /// Generates a new ID for this element. The id will not be in existing ids
        /// Updates the self element and the existing ids
        /// Returns the new id
        pub fn generate_id(&mut self, existing_ids: &mut Vec<u16>) -> u16 {
            let mut rng = rand::thread_rng();
            let mut new_id: u16 = 0;
            while new_id == 0 || existing_ids.iter().any(|&i| i==new_id) {
                new_id = rng.gen::<u16>();
            }
            self.id = Some(new_id);
            existing_ids.push(new_id);
            new_id
        }

        /// Returns the AppElement as an ListItem
        pub fn to_list_item<'a>(&self) -> ListItem<'a> {
            let mut item = ListItem::new(
                self
                    .title()
                    .unwrap_or("<no title>".to_string())
                    .clone()
            );
            if self.removed {
                item = item.style(Style::default().add_modifier(Modifier::CROSSED_OUT));
            } else if self.modified {
                item = item.style(Style::default().add_modifier(Modifier::BOLD));
            } else if self.id.is_none() {
                item = item.style(Style::default().add_modifier(Modifier::ITALIC));
            }
            item
        }
    }

    /// On which column the current acting focus is
    #[derive(Debug, PartialEq, Eq)]
    pub enum AppFocus {
        Elements,
        Attributes,
        Edit
    }

    impl AppFocus {
        /// Returns true if the current AppFocus is on Elements
        pub fn elements(&self) -> bool {
            self == &Self::Elements
        }
        /// Returns true if the current AppFocus is on Attributes
        pub fn attributes(&self) -> bool {
            self == &Self::Attributes
        }
        /// Returns true if the current AppFocus is on Edit
        pub fn edit(&self) -> bool {
            self == &Self::Edit
        }
    }

    /// The current state of the app
    pub struct AppState {
        config: AppConfig,
        client: Option<Client>,
        elements: Vec<EntryNode>,
        synced: bool,
        pub focused_on: AppFocus,
        pub list_state: ListState,
        pub details_state: TableState,
        pub prompt: Option<String>,
        pub message: Option<&'static str>,
        modification_buffer: Option<String>,
    }

    impl AppState {
        pub fn new(config: AppConfig) -> Self {
            Self {
                config,
                client: None,
                elements: Vec::new(),
                synced: false,
                focused_on: AppFocus::Elements,
                list_state: ListState::default(),
                details_state: TableState::default(),
                prompt: None,
                message: None,
                modification_buffer: None,
            }
        }

        /// Checks if we are currently Editing by checking whether the modification
        /// buffer is some
        pub fn is_editing(&self) -> bool {
            return self.modification_buffer.is_some();
        }

        /// Returns All AppElements from the state as a Vec
        pub fn get_elements(&self) -> Vec<EntryNode> {
            self.elements.clone()
        }

        /// Returns All AppElements mutable from the state as a Vec
        pub fn get_elements_mut(&mut self) -> &mut Vec<EntryNode> {
            &mut self.elements
        }

        /// Returns the currently selected Element if available
        pub fn get_selected_element(&self) -> Option<&EntryNode> {
            if let Some(indx) = self.list_state.selected() {
                self.elements.get(indx)
            } else {
                None
            }
        }

        /// Returns the currently selected Element as mutable if available
        pub fn get_selected_element_mut(&mut self) -> Option<&mut EntryNode> {
            if let Some(indx) = self.list_state.selected() {
                self.elements.get_mut(indx)
            } else {
                None
            }
        }

        /// Returns the currently selected attribute of the currently selected element
        /// if available
        pub fn get_selected_attribute(&self) -> Option<(NodeName, String)> {
            if let Some(element) = self.get_selected_element() {
                if let Some(indx) = self.details_state.selected() {
                    if let Some((k, v)) = element.get_vecs().get(indx) {
                        return Some((NodeName::from_str(&k), v.to_string()));
                    };
                }
            }
            None
        }

        /// Creates a new blank EntryNode, adds it to the current state and returns it
        pub fn create_new_element(self: &mut AppState) -> &mut EntryNode {
            let len = self.get_elements().len();
            let new_element: EntryNode = EntryNode::new(
                None,
                HashMap::new(),
            );
            self.push(Some(new_element));
            self.list_state.select(Some(len));
            return self.get_selected_element_mut().expect("FATAL Newly created element not found");
        }

        /// Gets the current Modification Buffer if we currently edit
        pub fn get_edit(&self) -> Option<String> {
            return self.modification_buffer.clone();
        }

        /// Sets the current Modification Buffer to the specified value
        /// returns whether the modification buffer is now Some or None
        pub fn set_edit(&mut self, value: Option<String>) -> bool {
            self.modification_buffer = value;
            self.modification_buffer.is_some()
        }

        /// Push the char value into the modifications buffer to the end
        pub fn push_edit(&mut self, value: char) {
            if let Some(buf) = &mut self.modification_buffer {
                buf.push(value);
            }
        }

        /// Removes the last char from the modifications buffer
        pub fn pop_edit(&mut self) {
            if let Some(buf) = &mut self.modification_buffer {
                buf.pop();
            }
        }

        /// Exits the editing mode and leaves everything untouchtd (hopefully)
        pub fn abort_editing(&mut self) {
            self.modification_buffer = None;
        }

        /// Creates a new attribute with empty value inside the currently selected
        /// element. The name of the new attribute will be the content of the
        /// current modification buffer. Skips if the buffer is None. Resets the
        /// buffer to None afterwards
        pub fn create_new_attribute_from_edit(self: &mut AppState) {
            if let Some(new_name) = &self.modification_buffer.clone() {
                if let Some(element) = self.get_selected_element_mut() {
                    element
                        .nodes()
                        .insert(
                            NodeName::from_str(&new_name),
                            NodeValue::Text("".to_string())
                        );
                    self.unsynced();
                }
            }
            self.modification_buffer = None;
        }

        /// Saves the current Modification Buffer to the currently selected node
        /// and exits the editing mode
        pub fn save_changes(self: &mut AppState) {
            let new_txt: String = self.get_edit().unwrap_or("".to_string());
            if let Some(node) = self.get_selected_attribute() {
                let element: &mut EntryNode = match self.get_selected_element_mut() {
                    Some(element) => element,
                    None => self.create_new_element(),
                };
                if Some(NodeValue::Text(new_txt.clone())) != element.nodes().insert(node.0, NodeValue::Text(new_txt)) {
                    element.modified();
                    self.unsynced();
                };
                self.modification_buffer = None;
            }
        }

        /// Finds and Returns an element defined by it's id
        pub fn get_element_by_id(&mut self, id: u16) -> Option<&mut EntryNode> {
            self
                .elements
                .iter_mut()
                .find(|e| e.id == Some(id))
        }

        /// Returns all IDs present in the current appstate
        pub fn get_ids(&self, ignore_removed: bool) -> Vec<u16> {
            return self.elements
                .clone()
                .into_iter()
                .filter(|e| !e.removed && ignore_removed)
                .filter_map(|e| e.id)
                .collect();

        }

        pub fn push(&mut self, element: Option<EntryNode>) {
            if let Some(e) = element {
                self.elements.push(e)
            }
        }

        pub fn unsynced(&mut self) {
            self.synced = false;
        }

        pub fn sort_by_due(&mut self) {
            self.elements.sort_by(|a, b| {
                match a.due() {
                    Some(due_a) => {
                        match b.due() {
                            Some(due_b) => {due_a.cmp(&due_b)},
                            None => {due_a.cmp(&0)}
                        }
                    },
                    None => {
                        match b.due() {
                            Some(due_b) => {due_b.cmp(&0)},
                            None => {0.cmp(&0)}
                        }
                    }
                }
            })
        }

        /// Returns a string that supposes to indicate whether modifications
        /// have been made to the local state
        pub fn modified_string(&self) -> String {
            if self.is_editing() {
                "editing"
            } else {
                match self.synced {
                    true => "synced",
                    false => "edited",
            }
            }.to_string()
        }

        fn handle_empty_client(&mut self) {
            if self.client.is_none() {
                self.client = Some(
                    Client::builder()
                        .user_agent("Freemind CLI")
                        .build().unwrap()
                );
            }
        }

        /// Adds non existing elements to the State of elements, skips
        /// already existing elements and returns the result
        fn add_new_elements(&mut self, new: Vec<&EntryNode>) {
            new.into_iter().for_each(|e| {
                if self.elements.iter().any(|i| e == i) {

                } else {
                    self.elements.push(e.clone())
                }
            })
        }

        /// Generates IDs for all elements in the current state that don't already
        /// have one. Needs a full list of existing IDs to avoid during generation
        fn add_missing_ids(&mut self, existing_ids: &mut Vec<u16>) -> (bool, Vec<u16>) {
            let mut new_ids: Vec<u16> = Vec::new();
            let count_after: usize = self.elements
                .iter_mut()
                .filter(|e| e.id.is_none())
                .map(|e| {
                    new_ids.push(e.generate_id(existing_ids))
                }).count();
            (count_after != 0, new_ids)
        }

        /// Makes a call to the configured server using the provided endpoint
        async fn call(&mut self, endpoint: &str, payload: String) -> Result<Response, reqwest::Error> {
            self.handle_empty_client();
            let res: Response = self.client.as_ref().unwrap()
                .post(format!("{}{}", self.config.server_address, endpoint))
                .header(
                    "user".to_string(),
                    HeaderValue::from_str(&self.config.username).unwrap()
                )
                .header(
                    format!("{}", &self.config.auth_method).to_lowercase(),
                    &self.config.secret
                )
                .header(
                    "content-type".to_string(),
                    "text/xml".to_string(),
                )
                .body(payload)
                .send()
                .await?;

            Ok(res)
        }

        /// Fetches the whole registry from the server
        async fn fetch(&mut self) -> Result<String, reqwest::Error> {
            let res: Response = self.call("/xml/fetch", "".to_string()).await?;

            let headers = res.headers();
            if headers.get("content-type") == Some(&HeaderValue::from_static("text/xml")) {
                let txt = res.text().await?;
                return Ok(txt);
            }

            Ok(String::new())
        }

        /// Uploads the given payload to the server and returns the HTTP status code
        async fn upload(&mut self, payload: String) -> Result<u16, reqwest::Error> {
            let res: Response = self.call("/xml/update", payload).await?;

            let status = res.status().as_u16();

            return Ok(status)
        }

        /// Takes the whole XML Document and removes all Entries that were removed
        /// in the internal state.
        /// Returns whether changes where made and the string of the new payload
        fn delete_removed(&mut self, xml: String) -> Result<(bool, String), quick_xml::Error> {
            let mut modified = false;

            let mut reader = Reader::from_str(&xml);
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            
            reader.trim_text(true);

            let mut ffwd: bool = false;
            let mut skip: BytesStart = BytesStart::new("");

            loop {
                match reader.read_event() {
                    Ok(Event::Start(_)) if ffwd => {
                        continue
                    }
                    Ok(Event::Start(e)) if e.name().as_ref() == b"entry" => {
                        let mut write = true;
                        if let Some(v) = get_id_attribute(&reader, &e) {
                            if let Some(pos) = self.elements.iter().position(|e| e.id == Some(v)) {
                                if self.elements[pos].removed {
                                    self.elements.remove(pos);
                                    ffwd = true;
                                    skip = e.to_owned();
                                    modified = true;
                                    write = false;
                                };
                            };
                        }
                        if write {
                            writer.write_event(Event::Start(e.to_owned()))?;
                        }
                    },
                    Ok(Event::Start(e)) => {
                        writer.write_event(Event::Start(e.to_owned()))?;
                    }
                    Ok(Event::End(e)) if e == skip.to_end() => {
                        ffwd = false;
                        skip = BytesStart::new("");
                    }
                    Ok(Event::End(_)) if ffwd => {
                        continue
                    }
                    Ok(Event::End(e)) => {
                        writer.write_event(Event::End(e.to_owned()))?;
                    },
                    Ok(Event::Eof) => break,
                    Ok(_) if ffwd => {
                        continue
                    },
                    Ok(e) => {
                        writer.write_event(e)?;
                    }
                    Err(_) => break,
                    //_ => (),
                }
            }

            Ok((
                modified,
                str::from_utf8(
                    &writer.into_inner().into_inner()
                )
                .unwrap()
                .to_string()
            ))
        }

        /// Takes the whole XML Document and inserts Entries defined by the ids vec into it
        fn insert_created_entries(&self, xml: String, ids: Vec<u16>) -> String {
            let mut reader = Reader::from_str(&xml);
            let mut writer = Writer::new(Cursor::new(Vec::new()));

            loop {
                match reader.read_event() {
                    Ok(Event::Start(e)) if e.name().as_ref() == b"registry" => {
                        writer.write_event(Event::Start(e.to_owned())).unwrap();
                        self.elements
                            .iter()
                            .filter(|e| ids.iter().any(|i| Some(i) == e.id.as_ref()))
                            .map(|e| {
                                e.write(&mut writer, true).unwrap();
                            }).count();
                    },
                    Ok(Event::Eof) => break,
                    Ok(e) => {writer.write_event(e).unwrap();}
                    Err(_) => break,
                }
            }

            str::from_utf8(
                &writer
                .into_inner()
                .into_inner()
            ).unwrap().to_string()
        }

        /// TODO: CHANGE THIS IT DOES NOT WORK PROPERLY ANYMORE
        /// Takes the whole XML Document and edits Entries that are marked to be
        /// in an edited state
        fn edit_entries(&mut self, xml: String) -> Result<(bool, String), quick_xml::Error> {
            let mut modified = false;

            let mut reader = Reader::from_str(&xml);
            let mut writer = Writer::new(Cursor::new(Vec::new()));

            let mut change_element: Option<u16> = None;
            let mut skip: BytesStart = BytesStart::new("");
            let mut skip_subtag: BytesStart = BytesStart::new("");

            loop {
                match reader.read_event() {
                    Ok(Event::Start(e)) => {
                        if let Some(element) = self.get_element_by_id(change_element.unwrap_or(0)) {
                            if element
                                .nodes
                                .contains_key(
                                    &NodeName::from_str(
                                        str::from_utf8(e
                                            .name()
                                            .as_ref()
                                        ).unwrap_or("")
                                    )
                                ) {
                                    skip_subtag = e.to_owned();
                                } else {
                                    if skip_subtag == BytesStart::new("") {
                                        writer.write_event(Event::Start(e.to_owned()))?
                                    }    
                                }
                        } else if e.name().as_ref() == b"entry" {
                            writer.write_event(Event::Start(e.to_owned())).unwrap();

                            if let Some(id) = get_id_attribute(&reader, &e) {
                                if let Some(element) = self.get_element_by_id(id) {
                                    if element.modified {
                                        element.modified = false;
                                        modified = true;
                                        change_element = Some(id);
                                        skip = e.to_owned();

                                        element.write(&mut writer, false)?;
                                    }
                                };
                            };
                        } else {
                            writer.write_event(Event::Start(e.to_owned())).unwrap();
                        }
                    },
                    Ok(Event::End(e)) if e == skip_subtag.to_end() => {
                        skip_subtag = BytesStart::new("");
                    }
                    Ok(Event::End(e)) if e == skip.to_end() => {
                        change_element = None;
                        skip = BytesStart::new("");
                        writer.write_event(Event::End(e.to_owned()))?;
                    },
                    Ok(Event::Eof) => break,
                    Ok(e) => if skip_subtag == BytesStart::new("") {
                        writer.write_event(e)?;
                    }
                    Err(_) => break,
                    //_ => (),
                }
            }

            Ok((
                modified,
                str::from_utf8(
                    &writer.into_inner().into_inner()
                )
                .unwrap()
                .to_string()
            ))
        }

        pub fn is_synced(&self) -> bool {
            self.synced
        }

        /// Syncs changes, fetches new elements, deletes removed elements and pushes
        pub async fn sync(&mut self) -> Result<(), reqwest::Error> {
            let result = self.fetch().await?;

            let (entries_deleted, mut answer) = self
                .delete_removed(result.to_string())
                .unwrap_or((false, result));

            //let entries_modified = false;
            let (entries_modified, mut answer) = self.edit_entries(answer).unwrap();

            let fetched_registry: Registry = Registry::empty().from_string(&answer).unwrap();

            let mut existing_ids: Vec<u16> = fetched_registry.entries()
                .clone()
                .into_iter()
                .filter(|e| !e.removed)
                .filter_map(|e| e.id)
                .collect();

            let (entries_added, new_ids) = self.add_missing_ids(&mut existing_ids);

            if entries_added {
                answer = self.insert_created_entries(answer, new_ids);
            }

            let needs_upload: bool = entries_deleted || entries_modified || entries_added;

            if needs_upload {
                self.upload(answer.clone()).await?;
            }


            self.add_new_elements(fetched_registry.entries());

            self.sort_by_due();

            self.synced = true;
            Ok(())
        }

        pub fn remove(&mut self, id: u16) -> bool {
            let Some(posi) = self.elements.iter().position(|e| e.id == Some(id)) else {return false};
            self.elements[posi].removed = true;
            true
        }

        /// Removes the currently selected element, returns true on successful removal
        pub fn remove_element(&mut self) -> bool {
            if let Some(index) = self.list_state.selected() {
                match self.elements.get_mut(index) {
                    Some(element) => {
                        element.removed = true;
                        return true;
                    }
                    _ => (),
                }
            }
            false
        }

        /// Removes the currently selected attribute from the currently selected element
        /// returns true on successful removal
        pub fn remove_attribute(&mut self) -> bool {
            if let Some(node) = self.get_selected_attribute() {
                if let Some(element) = self.get_selected_element_mut() {
                    match element.nodes().remove(&node.0) {
                        Some(_) => {
                            element.modified();
                            return true;
                        },
                        _ => (),
                    };
                }
            }

            false
        }
    }

    #[derive(PartialEq,)]
    pub enum AppCommand {
        Clear,      // C
        Edit,       // E
        Fill,       // F
        //Config,     // C
        Help,       // H
        Quit,       // Q
        Refresh,    // R
        None,
    }

    impl ToString for AppCommand {
        fn to_string(&self) -> String {
            match self {
                Self::Clear     => "[c]lear",
                Self::Fill      => "[f]ill with new",
                Self::Edit      => "[e]dit",
                Self::Refresh   => "[r]efresh",
                //Self::Config    => "[c]onfig",
                Self::Help      => "[h]elp",
                Self::Quit      => "[q]uit",
                Self::None      => "[n]one",
            }.to_string()
        }
    }

    impl From<usize> for AppCommand {
        fn from(s: usize) -> Self {
            match s {
                0 => Self::Refresh,
                1 => Self::Edit,
                2 => Self::Fill,
                3 => Self::Clear,
                4 => Self::Help,
                5 => Self::Quit,
                _ => Self::None,
            }
        }
    }

    impl AppCommand {
        pub fn get_command_list() -> Vec<Self> {
            let mut i: u8 = 0;
            let mut result: Vec<AppCommand> = Vec::new();
            loop {
                let e: AppCommand = AppCommand::from(i as usize); 
                if e == AppCommand::None {
                    break;
                }
                result.push(e);
                i += 1;
            }
            return result;
        }
        pub fn get_command_list_string() -> Vec<String> {
            Self::get_command_list().iter().map(|e| e.to_string()).collect()
        }
        pub fn from_key(key: KeyCode) -> Self {
            if let KeyCode::Char(val) = key {
                match val {
                    'c' => Self::Clear,
                    'e' => Self::Edit,
                    'f' => Self::Fill,
                    'h' => Self::Help,
                    'r' => Self::Refresh,
                    'q' => Self::Quit,
                    _ => Self::None,
                }
            } else {
                Self::None
            }
        }
    }

    #[derive(Serialize, Deserialize, PartialEq)]
    pub enum AuthMethod {
        Token,
        Password
    }

    impl From<usize> for AuthMethod {
        fn from(s: usize) -> AuthMethod {
            match s {
                0 => AuthMethod::Token,
                1 => AuthMethod::Password,
                _ => AuthMethod::Token,
            }
        }
    }

    impl fmt::Display for AuthMethod {
        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> fmt::Result {
            let displ: &str = match self {
                AuthMethod::Token => "Token",
                AuthMethod::Password => "Password",
            };
            write!(f, "{}", displ)
        }
    }

    #[derive(Serialize, Deserialize, PartialEq)]
    pub struct AppConfig {
        pub server_address: String,
        pub username: String,
        pub secret: String,
        pub auth_method: AuthMethod,
    }

    /// Construct a default AppConfig
    impl ::std::default::Default for AppConfig {
        fn default() -> Self {
            Self {
                server_address: "<THE ADDRESS OF THE WEBSERVER>".to_string(),
                username: "<YOUR USERNAME>".to_string(),
                secret: "<YOUR TOKEN / SECRET>".to_string(),
                auth_method: AuthMethod::Token,
            }
        }
    }

    impl fmt::Display for AppConfig {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "Server: {}\nUsername: {}\nSecret: {}\nAuth Method: {}",
                self.server_address, self.username, "*".repeat(self.secret.len()), self.auth_method
            )
        }
    }

    impl AppConfig {
        /// Returns if the element is the same as the default options
        pub(crate) fn is_default(&self) -> bool {
            self == &Self::default()
        }

        /// Returns if the element is the same as the empty element
        pub(crate) fn is_empty(&self) -> bool {
            self == &Self::empty()
        }

        /// Returns a minimal element
        pub(crate) fn empty() -> Self {
            Self {
                server_address: "".to_string(),
                username: "".to_string(),
                secret: "".to_string(),
                auth_method: AuthMethod::Token,
            }
        }

        pub(crate) fn new(server_address: String, username: String, secret: String, auth_method: AuthMethod) -> Self {
            Self {
                server_address,
                username,
                secret,
                auth_method,
            }
        }
    }
}