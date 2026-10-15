//! Machine controls share the normal immediate-mode UI and accessibility buttons.
use crate::*;
impl App {
    pub(crate) fn machine_screen(&mut self) {
        let Some(m)=self.game.machine.clone() else{return};let Some(spec)=machines::spec(m.block) else{return};
        let s=self.ui.s;let w=300.0*s;let h=(104.0+spec.actions.len() as f32*38.0)*s;
        let x=(screen_width()-w)*0.5;let y=((screen_height()-h)*0.5).max(4.0*s);
        draw_rectangle(0.0,0.0,screen_width(),screen_height(),Color::new(0.0,0.0,0.0,0.55));
        draw_rectangle(x,y,w,h,ui::panel());draw_rectangle_lines(x,y,w,h,s,WHITE);
        let title=block::item_name(m.block);*self.ui.title.borrow_mut()=title.to_string();
        self.ui.text(title,x+10.0*s,y+17.0*s,13.0,WHITE);
        self.ui.text(spec.hint,x+10.0*s,y+31.0*s,7.0,GRAY);
        self.ui.text(&m.status,x+10.0*s,y+45.0*s,7.0,Color::new(0.6,0.9,0.95,1.0));
        draw_rectangle(x+10.0*s,y+52.0*s,w-20.0*s,3.0*s,DARKGRAY);
        draw_rectangle(x+10.0*s,y+52.0*s,(w-20.0*s)*m.progress as f32/255.0,3.0*s,Color::new(0.3,0.8,0.65,1.0));
        for (i,a) in spec.actions.iter().enumerate() {
            let yy=y+(63.0+i as f32*38.0)*s;
            let ready=a.inputs.iter().all(|(key,n)|block::reg().lookup(key).is_some_and(|id|self.game.inv.count(id)>=*n));
            if self.ui.button(Rect::new(x+10.0*s,yy,w-20.0*s,21.0*s),a.label,ready) {self.game.machine_action(i as u8);}
            let cost=if a.inputs.is_empty(){"No ingredients".into()}else{a.inputs.iter().map(|(key,n)|{let id=block::reg().lookup(key).unwrap_or(0);format!("{}: {}/{n}",block::item_name(id),self.game.inv.count(id))}).collect::<Vec<_>>().join("    ")};
            self.ui.text(&cost,x+12.0*s,yy+31.0*s,7.0,if ready{GRAY}else{Color::new(1.0,0.65,0.45,1.0)});
        }
        if self.ui.button(Rect::new(x+w-76.0*s,y+h-27.0*s,66.0*s,19.0*s),"Close",true){self.set_screen(Screen::Playing);}
        self.ui.text("Esc / inventory to close",x+10.0*s,y+h-14.0*s,7.0,GRAY);
    }
}
