use super::{Space, SpaceId, page_resource::PageResource};



pub struct ImmortalSpace {
    id: SpaceId,
    pr: PageResource,
}

impl Space for ImmortalSpace {
    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: PageResource::new(id)
        }
    }

    #[inline(always)]
    fn id(&self) -> SpaceId {
        self.id
    }

    #[inline(always)]
    fn page_resource(&self) -> &PageResource {
        &self.pr
    }
}
